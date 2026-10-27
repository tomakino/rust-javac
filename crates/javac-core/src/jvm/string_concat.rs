// Source: com/sun/tools/javac/jvm/StringConcat.java

//! Lowers string concatenation to JVM-level operations: the legacy
//! StringBuilder chains or the invokedynamic StringConcatFactory calls.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::code::symtab::Symtab;
use crate::code::type_tag::TypeTag;

use crate::code::types::Types;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TreeId, TypeId};
use crate::jvm::byte_codes::*;
use crate::jvm::items::Item;
use crate::jvm::pool_constant::LoadableConstantRef;
use crate::jvm::target::Target;
use crate::util::constants::ConstantValue;
use crate::util::java_string::JavaString;
use crate::util::list::List;
use crate::util::names::Names;
use crate::tree::jc_tree::{Tag, TreeKind};
use crate::tree::tree_info;
use crate::tree::tree_maker::TreeMaker;

/// Maximum number of slots for a StringConcat call.
const MAX_INDY_CONCAT_ARG_SLOTS: i32 = 200;
/// The recipe tag for a dynamic argument.
const TAG_ARG: u16 = 0x0001;
/// The recipe tag for a detached constant.
const TAG_CONST: u16 = 0x0002;

/// The bytecode flavor of the concatenation lowering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConcatKind {
    /// The legacy StringBuilder chains.
    Inline,
    /// invokedynamic without special constant handling.
    IndyPlain,
    /// invokedynamic with constants in the recipe.
    IndyConstants,
}

/// The string concatenation lowering.
pub struct StringConcat {
    /// The symbol table.
    pub syms: Rc<RefCell<Symtab>>,
    /// The name table.
    pub names: Rc<Names>,
    /// The tree factory.
    pub make: Rc<RefCell<TreeMaker>>,
    /// Type relations.
    pub types: Rc<Types>,
    /// The StringBuilder.append methods, indexed by parameter type.
    pub sb_appends: HashMap<TypeId, SymbolId>,
    /// The selected bytecode flavor.
    pub kind: ConcatKind,
}

/// Create the concatenation lowering for the given options.
///
/// # Arguments
///   - gen: the code generator services;
///   - syms: the symbol table;
///   - names: the name table;
///   - make: the tree factory;
///   - types: the type relations;
///   - option: the value of the stringConcat option, if set;
///   - target: the code generation target.
///
/// # Returns
///   - the configured lowering.
///
/// # Panics
///   - the requested flavor is unknown or unsupported by the target.
pub fn make_concat(
    syms: Rc<RefCell<Symtab>>,
    names: Rc<Names>,
    make: Rc<RefCell<TreeMaker>>,
    types: Rc<Types>,
    option: Option<String>,
    target: &Target,
) -> StringConcat {
    let mut opt = option;
    if target.has_string_concat_factory() {
        if opt.is_none() {
            opt = Some("indyWithConstants".to_string());
        }
    } else {
        if let Some(value) = &opt
            && value != "inline"
        {
            panic!(
                "AssertionError: StringConcatFactory-based string concat is requested on a platform that does not support it."
            );
        }
        opt = Some("inline".to_string());
    }
    let kind = match opt.as_deref() {
        Some("inline") => ConcatKind::Inline,
        Some("indy") => ConcatKind::IndyPlain,
        Some("indyWithConstants") => ConcatKind::IndyConstants,
        _ => panic!("AssertionError: Unknown stringConcat: {:?}", opt),
    };
    StringConcat {
        syms,
        names,
        make,
        types,
        sb_appends: HashMap::new(),
        kind,
    }
}

impl StringConcat {
    /// Lower a string compound assignment.
    pub fn make_concat_assignop(
        &mut self,
        ctx: &mut CompilerContext,
        generator: &mut crate::jvm::codegen::Gen,
        tree: TreeId,
    ) -> Item {
        match self.kind {
            ConcatKind::Inline => self.make_concat_assignop_inline(ctx, generator, tree),
            ConcatKind::IndyPlain | ConcatKind::IndyConstants => {
                self.make_concat_assignop_indy(ctx, generator, tree)
            }
        }
    }

    /// Lower a string addition expression.
    pub fn make_concat_binary(&mut self,
        ctx: &mut CompilerContext,
        generator: &mut crate::jvm::codegen::Gen, tree: TreeId) -> Item {
        match self.kind {
            ConcatKind::Inline => self.make_concat_binary_inline(ctx, generator, tree),
            ConcatKind::IndyPlain => self.make_concat_binary_indy(ctx, generator, tree, false),
            ConcatKind::IndyConstants => self.make_concat_binary_indy(ctx, generator, tree, true),
        }
    }

    /// Collect all concatenated expressions of a tree.
    fn collect_all(&self, ctx: &CompilerContext, tree: TreeId) -> List<TreeId> {
        self.collect(ctx, tree, List::nil())
    }

    /// Collect all concatenated expressions of two trees.
    fn collect_all_two(
        &self,
        ctx: &CompilerContext,
        lhs: TreeId,
        rhs: TreeId,
    ) -> List<TreeId> {
        let mut result = self.collect(ctx, lhs, List::nil());
        result = self.collect(ctx, rhs, result);
        result
    }

    /// Collect the concatenated sub-expressions of a tree into res.
    fn collect(
        &self,
        ctx: &CompilerContext,
        tree: TreeId,
        res: List<TreeId>,
    ) -> List<TreeId> {
        let tree = tree_info::skip_parens(ctx, tree);
        let node = ctx.tree(tree);
        if node.get_tag() == Tag::PLUS
            && node.ty.is_none_or(|ty| ctx.typ(ty).const_value().is_none())
            && let TreeKind::Binary {
                lhs,
                rhs,
                operator,
            } = &node.kind
        {
                let mut is_string_add = false;
                if let Some(operator) = operator
                    && ctx.sym(*operator).kind == crate::code::kinds::Kind::MTH
                    && let crate::code::symbol::SymbolKind::OperatorSymbol { opcode, .. } =
                        &ctx.sym(*operator).kind_data
                {
                    is_string_add = *opcode == STRING_ADD;
                }
                if is_string_add {
                    let mut result = res;
                    result = self.collect(
                        ctx,
                        lhs.expect("binary lhs"),
                        result,
                    );
                    result = self.collect(
                        ctx,
                        rhs.expect("binary rhs"),
                        result,
                    );
                    return result;
                }
            }
        res.append(tree)
    }
}

impl StringConcat {
    /// Lower a string compound assignment with StringBuilder chains.
    fn make_concat_assignop_inline(
        &mut self,
        ctx: &mut CompilerContext,
        generator: &mut crate::jvm::codegen::Gen,
        tree: TreeId,
    ) -> Item {
        let pos = ctx.tree(tree).pos;
        let (lhs, rhs) = assign_op_parts(ctx, tree);
        // Create a string builder.
        self.new_string_builder(ctx, generator, tree);
        // Generate code for the first string, possibly save one copy under
        // the builder.
        let lhs_ty = ctx.tree(lhs).ty.expect("lhs type");
        let l = generator.gen_expr(ctx, lhs, lhs_ty);
        let width = generator.get_items().borrow().width(&l);
        if width > 0 {
            let code = generator.get_code();
            code.borrow_mut().emitop0(ctx, DUP_X1 + 3 * (width - 1));
        }
        generator.get_items().borrow_mut().load(ctx, &l);
        self.append_string(ctx, generator, lhs);
        // Append all other strings to the builder.
        let args = self.collect_all(ctx, rhs);
        let mut current = args;
        while current.non_empty() {
            let t = *current.head();
            current = current.tail();
            let ty = ctx.tree(t).ty.expect("argument type");
            let item = generator.gen_expr(ctx, t, ty);
            generator.get_items().borrow_mut().load(ctx, &item);
            self.append_string(ctx, generator, t);
        }
        self.builder_to_string(ctx, generator, pos);
        l
    }

    /// Lower a string addition with StringBuilder chains.
    fn make_concat_binary_inline(&mut self,
        ctx: &mut CompilerContext,
        generator: &mut crate::jvm::codegen::Gen, tree: TreeId) -> Item {
        let pos = ctx.tree(tree).pos;
        self.new_string_builder(ctx, generator, tree);
        let args = self.collect_all(ctx, tree);
        let mut current = args;
        while current.non_empty() {
            let t = *current.head();
            current = current.tail();
            let ty = ctx.tree(t).ty.expect("argument type");
            let item = generator.gen_expr(ctx, t, ty);
            generator.get_items().borrow_mut().load(ctx, &item);
            self.append_string(ctx, generator, t);
        }
        self.builder_to_string(ctx, generator, pos);
        let syms = Rc::clone(&self.syms);
        let items = generator.get_items();
        items.borrow().make_stack_item(ctx, syms.borrow().string_type)
    }

    /// Emit the creation of a new StringBuilder.
    fn new_string_builder(&mut self,
        ctx: &mut CompilerContext,
        generator: &mut crate::jvm::codegen::Gen, tree: TreeId) -> i32 {
        let pos = ctx.tree(tree).pos;
        let sb_type = self.syms.borrow().string_builder_type;
        let od = generator.make_ref(ctx, pos, sb_type);
        {
            let code = generator.get_code();
            let mut code = code.borrow_mut();
            code.emitop2_type(ctx, NEW, od, sb_type);
            code.emitop0(ctx, DUP);
        }
        generator.call_method(
            ctx,
            pos,
            sb_type,
            self.names.init.clone(),
            List::nil(),
            false,
        );
        pos
    }

    /// Append the string value of a tree to the top StringBuilder.
    fn append_string(&mut self,
        ctx: &mut CompilerContext,
        generator: &mut crate::jvm::codegen::Gen, tree: TreeId) {
        let tree_ty = ctx.tree(tree).ty.expect("tree type");
        let t = self.types.base_type(ctx, tree_ty);
        let string_tsym = ctx.typ(self.syms.borrow().string_type).tsym.expect("string type symbol");
        let is_primitive = ctx.typ(t).get_tag().is_primitive();
        let t_tsym = ctx.typ(t).tsym;
        let mut t = t;
        if !is_primitive && t_tsym != Some(string_tsym) {
            t = self.syms.borrow().object_type;
        }
        assert!(
            ctx.typ(t).const_value().is_none(),
            "AssertionError: null constant type"
        );
        let method = match self.sb_appends.get(&t) {
            Some(method) => *method,
            None => {
                let env = generator.get_attr_env();
                let pos = ctx.tree(tree).pos;
                let sb_type = self.syms.borrow().string_builder_type;
                let append_name = self.names.append.clone();
                let method = generator.resolve_internal_method(
                    ctx,
                    pos,
                    &env,
                    sb_type,
                    append_name,
                    List::of(t),
                );
                self.sb_appends.insert(t, method);
                method
            }
        };
        let items = generator.get_items();
        let item = items.borrow_mut().make_member_item(ctx, method, false);
        items.borrow_mut().invoke(ctx, &item);
    }

    /// Convert the top StringBuilder to a String.
    fn builder_to_string(&mut self,
        ctx: &mut CompilerContext,
        generator: &mut crate::jvm::codegen::Gen, pos: i32) {
        let sb_type = self.syms.borrow().string_builder_type;
        let name = self.names.toString.clone();
        generator.call_method(ctx, pos, sb_type, name, List::nil(), false);
    }

    /// Lower a string compound assignment with invokedynamic.
    fn make_concat_assignop_indy(
        &mut self,
        ctx: &mut CompilerContext,
        generator: &mut crate::jvm::codegen::Gen,
        tree: TreeId,
    ) -> Item {
        let pos = ctx.tree(tree).pos;
        let (lhs, rhs) = assign_op_parts(ctx, tree);
        let args = self.collect_all_two(ctx, lhs, rhs);
        let lhs_ty = ctx.tree(lhs).ty.expect("lhs type");
        let l = generator.gen_expr(ctx, lhs, lhs_ty);
        {
            let items = generator.get_items();
            items.borrow_mut().duplicate(ctx, &l);
            items.borrow_mut().load(ctx, &l);
        }
        let tree_ty = ctx.tree(tree).ty.expect("assign type");
        self.emit_indy(ctx, generator, pos, args, false, tree_ty);
        l
    }

    /// Lower a string addition with invokedynamic.
    fn make_concat_binary_indy(
        &mut self,
        ctx: &mut CompilerContext,
        generator: &mut crate::jvm::codegen::Gen,
        tree: TreeId,
        _with_constants: bool,
    ) -> Item {
        let pos = ctx.tree(tree).pos;
        let (lhs, rhs) = binary_parts(ctx, tree);
        let args = self.collect_all_two(ctx, lhs, rhs);
        let tree_ty = ctx.tree(tree).ty.expect("binary type");
        self.emit_indy(ctx, generator, pos, args, true, tree_ty);
        let syms = Rc::clone(&self.syms);
        let items = generator.get_items();
        items.borrow().make_stack_item(ctx, syms.borrow().string_type)
    }

    /// Emit the invokedynamic call for the collected arguments.
    fn emit_indy(
        &mut self,
        ctx: &mut CompilerContext,
        generator: &mut crate::jvm::codegen::Gen,
        pos: i32,
        args: List<TreeId>,
        generate_first_arg: bool,
        ty: TypeId,
    ) {
        match self.kind {
            ConcatKind::IndyPlain => {
                self.emit_plain(ctx, generator, pos, args, generate_first_arg, ty)
            }
            ConcatKind::IndyConstants => {
                self.emit_constants(ctx, generator, pos, args, generate_first_arg, ty)
            }
            ConcatKind::Inline => panic!("AssertionError: inline concat has no indy call"),
        }
    }

    /// Peel the argument list into smaller chunks.
    fn split(&self, ctx: &CompilerContext, args: List<TreeId>) -> Vec<Vec<TreeId>> {
        let mut splits: Vec<Vec<TreeId>> = Vec::new();
        let mut slots = 0;
        let mut current: Vec<TreeId> = Vec::new();
        let mut rest = args;
        while rest.non_empty() {
            let t = *rest.head();
            rest = rest.tail();
            let tag = ctx
                .tree(t)
                .ty
                .map(|ty| ctx.typ(ty).get_tag())
                .expect("argument type");
            let need_slots = if tag == TypeTag::LONG || tag == TypeTag::DOUBLE {
                2
            } else {
                1
            };
            if slots + need_slots >= MAX_INDY_CONCAT_ARG_SLOTS {
                splits.push(std::mem::take(&mut current));
                slots = 0;
            }
            current.push(t);
            slots += need_slots;
        }
        if !current.is_empty() {
            splits.push(current);
        }
        splits
    }

    /// Return whether an argument must be converted to a String eagerly to
    /// preserve side effects.
    fn should_convert_to_string_eagerly(
        &mut self,
        ctx: &mut CompilerContext,
        _generator: &mut crate::jvm::codegen::Gen,
        arg_type: TypeId,
    ) -> bool {
        let unboxed = self.types.unboxed_type_ro(ctx, &self.syms.borrow(), arg_type);
        let string_tsym = ctx.typ(self.syms.borrow().string_type).tsym.expect("string type symbol");
        !ctx.typ(unboxed).get_tag().is_primitive()
            && ctx.typ(arg_type).tsym != Some(string_tsym)
    }

    /// Emit the plain invokedynamic concat.
    fn emit_plain(
        &mut self,
        ctx: &mut CompilerContext,
        generator: &mut crate::jvm::codegen::Gen,
        pos: i32,
        args: List<TreeId>,
        generate_first_arg: bool,
        ty: TypeId,
    ) {
        let split = self.split(ctx, args);
        let mut first = true;
        for slice in split.clone() {
            assert!(!slice.is_empty(), "AssertionError: Arguments list is empty");
            let mut dynamic_args: Vec<TypeId> = Vec::new();
            for arg in slice {
                let const_val = ctx
                    .tree(arg)
                    .ty
                    .and_then(|ty| ctx.typ(ty).const_value().cloned());
                if matches!(&const_val, Some(ConstantValue::String(s)) if s.is_empty()) {
                    continue;
                }
                let arg_ty = ctx.tree(arg).ty.expect("argument type");
                let mut arg_type = arg_ty;
                if arg_type == self.syms.borrow().bot_type {
                    let boxed = self.types.boxed_class_ro(ctx, &self.syms.borrow(), self.syms.borrow().void_type);
                    arg_type = ctx.sym(boxed).ty.expect("boxed class type");
                }
                if !first || generate_first_arg {
                    let item = generator.gen_expr(ctx, arg, arg_ty);
                    generator.get_items().borrow_mut().load(ctx, &item);
                }
                if self.should_convert_to_string_eagerly(ctx, generator, arg_type) {
                    let value_of = self.names.valueOf.clone();
                    let object_type = self.syms.borrow().object_type;
                    let string_type = self.syms.borrow().string_type;
                    let argtypes = List::of(object_type);
                    generator.call_method(
                        ctx, pos, string_type, value_of, argtypes, true,
                    );
                    arg_type = self.syms.borrow().string_type;
                }
                dynamic_args.push(arg_type);
                first = false;
            }
            self.do_call_plain(ctx, generator, ty, pos, dynamic_args);
        }
        if split.len() > 1 {
            let mut arg_types: Vec<TypeId> = Vec::new();
            for _ in 0..split.len() {
                arg_types.push(self.syms.borrow().string_type);
            }
            self.do_call_plain(ctx, generator, ty, pos, arg_types);
        }
    }

    /// Emit the actual invokedynamic call without constant arguments.
    fn do_call_plain(
        &mut self,
        ctx: &mut CompilerContext,
        generator: &mut crate::jvm::codegen::Gen,
        ty: TypeId,
        pos: i32,
        dynamic_arg_types: Vec<TypeId>,
    ) {
        let indy_type = make_method_type(ctx, &dynamic_arg_types, ty, self.syms.borrow().method_class);
        let prev_pos = self.make.borrow().pos;
        self.make.borrow_mut().at(pos);
        let bsm_static_args = List::of(self.syms.borrow().method_handle_lookup_type)
            .append(self.syms.borrow().string_type)
            .append(self.syms.borrow().method_type_type);
        let env = generator.get_attr_env();
        let make_concat_name = self.names.makeConcat.clone();
        let concat_factory = self.syms.borrow().string_concat_factory;
        let bsm = generator.resolve_internal_method(
            ctx,
            pos,
            &env,
            concat_factory,
            make_concat_name.clone(),
            bsm_static_args.clone(),
        );
        let handle = crate::code::symbol::as_method_handle(ctx, bsm, false);
        let no_symbol = self.syms.borrow().no_symbol;
        let dyn_sym = crate::code::symbol::new_dynamic_method_symbol(
            ctx,
            make_concat_name,
            no_symbol,
            handle,
            indy_type,
            Vec::new(),
        );
        let items_rc = generator.get_items();
        let item = items_rc.borrow_mut().make_dynamic_item(ctx, dyn_sym);
        items_rc.borrow_mut().invoke(ctx, &item);
        self.make.borrow_mut().at(prev_pos);
    }

    /// Emit the invokedynamic concat with constants folded into the recipe.
    fn emit_constants(
        &mut self,
        ctx: &mut CompilerContext,
        generator: &mut crate::jvm::codegen::Gen,
        pos: i32,
        args: List<TreeId>,
        generate_first_arg: bool,
        ty: TypeId,
    ) {
        let split = self.split(ctx, args);
        let mut first = true;
        for slice in split.clone() {
            assert!(!slice.is_empty(), "AssertionError: Arguments list is empty");
            let mut recipe: Vec<u16> = Vec::new();
            let mut dynamic_args: Vec<TypeId> = Vec::new();
            let mut static_args: Vec<LoadableConstantRef> = Vec::new();
            for arg in slice {
                let const_val = ctx
                    .tree(arg)
                    .ty
                    .and_then(|ty| ctx.typ(ty).const_value().cloned());
                let arg_ty = ctx.tree(arg).ty.expect("argument type");
                if matches!(&const_val, Some(ConstantValue::String(s)) if s.is_empty()) {
                    continue;
                }
                if arg_ty == self.syms.borrow().bot_type {
                    // Concat the null into the recipe right away.
                    let null_units = JavaString::from("null");
                    recipe.extend_from_slice(null_units.as_units());
                } else if const_val.is_some() {
                    // Concat the String representation of the constant, except
                    // when it contains special tags (needs a detached constant).
                    let a = self.types.string_value(ctx, arg_ty);
                    let units = a.as_units();
                    if units.contains(&TAG_CONST) || units.contains(&TAG_ARG) {
                        recipe.push(TAG_CONST);
                        static_args.push(LoadableConstantRef::Basic(
                            crate::jvm::pool_constant::BasicConstant::string(a.clone()),
                        ));
                    } else {
                        recipe.extend_from_slice(units);
                    }
                } else {
                    // Ordinary arguments come through the dynamic arguments.
                    recipe.push(TAG_ARG);
                    let mut arg_type = arg_ty;
                    if !first || generate_first_arg {
                        let item = generator.gen_expr(ctx, arg, arg_ty);
                        generator.get_items().borrow_mut().load(ctx, &item);
                    }
                    if self.should_convert_to_string_eagerly(ctx, generator, arg_type) {
                        let value_of = self.names.valueOf.clone();
                        let object_type = self.syms.borrow().object_type;
                        let string_type = self.syms.borrow().string_type;
                        let argtypes = List::of(object_type);
                        generator.call_method(
                            ctx, pos, string_type, value_of, argtypes, true,
                        );
                        arg_type = self.syms.borrow().string_type;
                    }
                    dynamic_args.push(arg_type);
                    first = false;
                }
            }
            let recipe = JavaString::from_units(recipe);
            self.do_call_constants(ctx, generator, ty, pos, recipe, static_args, dynamic_args);
        }
        if split.len() > 1 {
            let mut arg_types: Vec<TypeId> = Vec::new();
            let mut recipe: Vec<u16> = Vec::new();
            for _ in 0..split.len() {
                arg_types.push(self.syms.borrow().string_type);
                recipe.push(TAG_ARG);
            }
            let recipe = JavaString::from_units(recipe);
            self.do_call_constants(ctx, generator, ty, pos, recipe, Vec::new(), arg_types);
        }
    }

    /// Emit the actual invokedynamic call with constant arguments.
    #[allow(clippy::too_many_arguments)]
    fn do_call_constants(
        &mut self,
        ctx: &mut CompilerContext,
        generator: &mut crate::jvm::codegen::Gen,
        ty: TypeId,
        pos: i32,
        recipe: JavaString,
        static_args: Vec<LoadableConstantRef>,
        dynamic_arg_types: Vec<TypeId>,
    ) {
        let indy_type = make_method_type(ctx, &dynamic_arg_types, ty, self.syms.borrow().method_class);
        let prev_pos = self.make.borrow().pos;
        self.make.borrow_mut().at(pos);
        let mut const_types: Vec<TypeId> = Vec::new();
        let mut constants: Vec<LoadableConstantRef> = Vec::new();
        for t in &static_args {
            constants.push(t.clone());
            const_types.push(self.syms.borrow().string_type);
        }
        let mut bsm_static_args = List::of(self.syms.borrow().method_handle_lookup_type)
            .append(self.syms.borrow().string_type)
            .append(self.syms.borrow().method_type_type)
            .append(self.syms.borrow().string_type);
        for const_type in const_types {
            bsm_static_args = bsm_static_args.append(const_type);
        }
        let env = generator.get_attr_env();
        let name = self.names.makeConcatWithConstants.clone();
        let concat_factory = self.syms.borrow().string_concat_factory;
        let bsm = generator.resolve_internal_method(
            ctx,
            pos,
            &env,
            concat_factory,
            name.clone(),
            bsm_static_args.clone(),
        );
        let handle = crate::code::symbol::as_method_handle(ctx, bsm, false);
        let no_symbol = self.syms.borrow().no_symbol;
        let mut all_static_args: Vec<LoadableConstantRef> = Vec::new();
        all_static_args.push(LoadableConstantRef::Basic(
            crate::jvm::pool_constant::BasicConstant::string(recipe),
        ));
        all_static_args.extend(constants);
        let dyn_sym = crate::code::symbol::new_dynamic_method_symbol(
            ctx,
            name,
            no_symbol,
            handle,
            indy_type,
            all_static_args,
        );
        let items_rc = generator.get_items();
        let item = items_rc.borrow_mut().make_dynamic_item(ctx, dyn_sym);
        items_rc.borrow_mut().invoke(ctx, &item);
        self.make.borrow_mut().at(prev_pos);
    }
}

/// Return the lhs/rhs trees of a compound assignment.
fn assign_op_parts(ctx: &CompilerContext, tree: TreeId) -> (TreeId, TreeId) {
    match &ctx.tree(tree).kind {
        TreeKind::AssignOp { lhs, rhs, .. } => (
            lhs.expect("assignop lhs"),
            rhs.expect("assignop rhs"),
        ),
        _ => panic!("AssertionError: compound assignment expected"),
    }
}

/// Return the lhs/rhs trees of a binary expression.
fn binary_parts(ctx: &CompilerContext, tree: TreeId) -> (TreeId, TreeId) {
    match &ctx.tree(tree).kind {
        TreeKind::Binary { lhs, rhs, .. } => {
            (lhs.expect("binary lhs"), rhs.expect("binary rhs"))
        }
        _ => panic!("AssertionError: binary expression expected"),
    }
}

/// Create a method type with the given argument and return types.
fn make_method_type(
    ctx: &mut CompilerContext,
    argtypes: &[TypeId],
    restype: TypeId,
    method_class: SymbolId,
) -> TypeId {
    let mut args = List::nil();
    for arg in argtypes {
        args = args.append(*arg);
    }
    ctx.alloc_type(crate::code::type_::Type::new(
        crate::code::type_tag::TypeTag::METHOD,
        Some(method_class),
        crate::code::type_::TypeKindData::MethodType {
            argtypes: args,
            restype: Some(restype),
            thrown: List::nil(),
            recvtype: None,
        },
    ))
}
