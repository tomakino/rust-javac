// Source: com/sun/tools/javac/jvm/Items.java

//! Code generation items: objects that stand for addressable entities in the
//! bytecode and implement the load/store/invoke protocol.

use std::cell::RefCell;
use std::rc::Rc;

use crate::code::symtab::Symtab;
use crate::code::types::Types;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TreeId, TypeId};
use crate::jvm::byte_codes::*;
use crate::jvm::code::{Chain, Code};
use crate::jvm::cr_table::CrtKey;
use crate::jvm::crt_flags::{CRT_BRANCH_FALSE, CRT_BRANCH_TRUE};
use crate::jvm::pool_constant::{
    loadable_to_pool_constant, BasicConstant, LoadableConstantRef, PoolKey,
};
use crate::jvm::pool_writer::PoolWriter;
use crate::util::java_string::JavaString;

/// A literal value carried by an immediate item.
#[derive(Clone, Debug)]
pub enum ImmediateValue {
    /// An int-compatible literal.
    Int(i32),
    /// A long literal.
    Long(i64),
    /// A float literal.
    Float(f32),
    /// A double literal.
    Double(f64),
    /// A String literal.
    String(JavaString),
}

/// The kind-specific data of an item.
#[derive(Clone, Debug)]
pub enum ItemKind {
    /// A value that never appears on the stack.
    Void,
    /// A value on the operand stack.
    Stack,
    /// An indexed expression (array access).
    Indexed,
    /// The this or super reference.
    SelfItem {
        /// Whether this item represents super.
        is_super: bool,
    },
    /// A local variable.
    Local {
        /// The register of the variable.
        reg: i32,
        /// The type of the variable.
        type_: TypeId,
    },
    /// A static variable or method.
    Static {
        /// The represented symbol.
        member: SymbolId,
    },
    /// A dynamic call site or constant.
    Dynamic {
        /// The represented symbol.
        member: SymbolId,
    },
    /// An instance variable or method.
    Member {
        /// The represented symbol.
        member: SymbolId,
        /// Whether the access is non-virtual.
        nonvirtual: bool,
    },
    /// A literal.
    Immediate {
        /// The literal value.
        value: LoadableConstantRef,
    },
    /// An assignment expression.
    Assign {
        /// The item representing the left hand side.
        lhs: Box<Item>,
    },
    /// A conditional or unconditional jump.
    Cond {
        /// The jump opcode.
        opcode: i32,
        /// The jumps taken when the condition is true.
        true_jumps: Option<Box<Chain>>,
        /// The jumps taken when the condition is false.
        false_jumps: Option<Box<Chain>>,
        /// The tree of the condition, used for the character range table.
        tree: Option<TreeId>,
    },
}

/// An item representing an addressable entity in the bytecode.
#[derive(Clone, Debug)]
pub struct Item {
    /// The type code of the values represented by this item.
    pub typecode: i32,
    /// The kind-specific data.
    pub kind: ItemKind,
}

impl Item {
    /// Create an item of the given kind and type code.
    pub fn new(typecode: i32, kind: ItemKind) -> Item {
        Item { typecode, kind }
    }
}

/// The item factory and the code generation protocol of items.
pub struct Items {
    /// The current constant pool writer.
    pub pool_writer: Rc<RefCell<PoolWriter>>,
    /// The current code buffer.
    pub code: Rc<RefCell<Code>>,
    /// The current symbol table.
    pub syms: Rc<RefCell<Symtab>>,
    /// Type utilities.
    pub types: Rc<Types>,
    /// The flyweight void item.
    void_item: Item,
    /// The flyweight this item.
    this_item: Item,
    /// The flyweight super item.
    super_item: Item,
    /// The flyweight stack items, indexed by type code.
    stack_item: Vec<Item>,
}

impl Items {
    /// Create the item factory for one method.
    ///
    /// # Arguments
    ///   - pool_writer: the constant pool writer;
    ///   - code: the code buffer;
    ///   - syms: the symbol table;
    ///   - types: the type relations.
    ///
    /// # Returns
    ///   - the initialized factory.
    ///
    /// # Panics
    ///   - none.
    pub fn new(
        pool_writer: Rc<RefCell<PoolWriter>>,
        code: Rc<RefCell<Code>>,
        syms: Rc<RefCell<Symtab>>,
        types: Rc<Types>,
    ) -> Items {
        let void_item = Item::new(VOID_CODE, ItemKind::Void);
        let this_item = Item::new(OBJECT_CODE, ItemKind::SelfItem { is_super: false });
        let super_item = Item::new(OBJECT_CODE, ItemKind::SelfItem { is_super: true });
        let mut stack_item = Vec::new();
        for i in 0..VOID_CODE {
            stack_item.push(Item::new(i, ItemKind::Stack));
        }
        stack_item.push(void_item.clone());
        Items {
            pool_writer,
            code,
            syms,
            types,
            void_item,
            this_item,
            super_item,
            stack_item,
        }
    }

    /// Make a void item.
    pub fn make_void_item(&self) -> Item {
        self.void_item.clone()
    }

    /// Make an item representing this.
    pub fn make_this_item(&self) -> Item {
        self.this_item.clone()
    }

    /// Make an item representing super.
    pub fn make_super_item(&self) -> Item {
        self.super_item.clone()
    }

    /// Make an item representing a value on the stack.
    pub fn make_stack_item(&self, ctx: &CompilerContext, type_: TypeId) -> Item {
        self.stack_item[Code::typecode(ctx, type_) as usize].clone()
    }

    /// Make an item representing a dynamic call site.
    pub fn make_dynamic_item(&mut self, ctx: &mut CompilerContext, member: SymbolId) -> Item {
        let ty = ctx.sym(member).ty.expect("member type");
        let types = Rc::clone(&self.types);
        let syms = Rc::clone(&self.syms);
        let erased = types.erasure(ctx, &syms.borrow(), ty);
        Item::new(
            Code::typecode(ctx, erased),
            ItemKind::Dynamic { member },
        )
    }

    /// Make an item representing an indexed expression.
    pub fn make_indexed_item(&self, ctx: &CompilerContext, type_: TypeId) -> Item {
        Item::new(Code::typecode(ctx, type_), ItemKind::Indexed)
    }

    /// Make an item representing a local variable.
    pub fn make_local_item(&mut self, ctx: &mut CompilerContext, v: SymbolId) -> Item {
        let ty = ctx.sym(v).ty.expect("variable type");
        let types = Rc::clone(&self.types);
        let syms = Rc::clone(&self.syms);
        let erased = types.erasure(ctx, &syms.borrow(), ty);
        let reg = match &ctx.sym(v).kind_data {
            crate::code::symbol::SymbolKind::VarSymbol { adr, .. } => *adr,
            _ => panic!("AssertionError: variable symbol expected"),
        };
        Item::new(Code::typecode(ctx, erased), ItemKind::Local { reg, type_: erased })
    }

    /// Make an item representing a local anonymous variable.
    pub fn make_local_item_anonymous(
        &self,
        ctx: &CompilerContext,
        type_: TypeId,
        reg: i32,
    ) -> Item {
        Item::new(Code::typecode(ctx, type_), ItemKind::Local { reg, type_ })
    }

    /// Make an item representing a static variable or method.
    pub fn make_static_item(&mut self, ctx: &mut CompilerContext, member: SymbolId) -> Item {
        let ty = ctx.sym(member).ty.expect("member type");
        let types = Rc::clone(&self.types);
        let syms = Rc::clone(&self.syms);
        let erased = types.erasure(ctx, &syms.borrow(), ty);
        Item::new(
            Code::typecode(ctx, erased),
            ItemKind::Static { member },
        )
    }

    /// Make an item representing an instance variable or method.
    pub fn make_member_item(
        &mut self,
        ctx: &mut CompilerContext,
        member: SymbolId,
        nonvirtual: bool,
    ) -> Item {
        let ty = ctx.sym(member).ty.expect("member type");
        let types = Rc::clone(&self.types);
        let syms = Rc::clone(&self.syms);
        let erased = types.erasure(ctx, &syms.borrow(), ty);
        Item::new(
            Code::typecode(ctx, erased),
            ItemKind::Member {
                member,
                nonvirtual,
            },
        )
    }

    /// Make an item representing a literal.
    pub fn make_immediate_item(
        &self,
        ctx: &CompilerContext,
        type_: TypeId,
        value: ImmediateValue,
    ) -> Item {
        let typecode = Code::typecode(ctx, type_);
        let constant = match typecode {
            BYTE_CODE | SHORT_CODE | CHAR_CODE | INT_CODE => match value {
                ImmediateValue::Int(v) => BasicConstant::int(v),
                _ => panic!("AssertionError: int literal expected"),
            },
            LONG_CODE => match value {
                ImmediateValue::Long(v) => BasicConstant::long(v),
                _ => panic!("AssertionError: long literal expected"),
            },
            FLOAT_CODE => match value {
                ImmediateValue::Float(v) => BasicConstant::float(v),
                _ => panic!("AssertionError: float literal expected"),
            },
            DOUBLE_CODE => match value {
                ImmediateValue::Double(v) => BasicConstant::double(v),
                _ => panic!("AssertionError: double literal expected"),
            },
            OBJECT_CODE => match value {
                ImmediateValue::String(v) => BasicConstant::string(v),
                _ => panic!("AssertionError: string literal expected"),
            },
            _ => panic!("UnsupportedOperationException: unsupported tag: {}", typecode),
        };
        Item::new(
            typecode,
            ItemKind::Immediate {
                value: LoadableConstantRef::Basic(constant),
            },
        )
    }

    /// Make an item representing an assignment expression.
    pub fn make_assign_item(&self, lhs: Item) -> Item {
        let typecode = lhs.typecode;
        Item::new(
            typecode,
            ItemKind::Assign {
                lhs: Box::new(lhs),
            },
        )
    }

    /// Make an item representing a conditional or unconditional jump.
    pub fn make_cond_item(
        &self,
        opcode: i32,
        true_jumps: Option<Box<Chain>>,
        false_jumps: Option<Box<Chain>>,
    ) -> Item {
        Item::new(
            BYTE_CODE,
            ItemKind::Cond {
                opcode,
                true_jumps,
                false_jumps,
                tree: None,
            },
        )
    }
}

impl Items {
    /// Generate code to load an item onto the stack and return the stack item.
    pub fn load(&mut self, ctx: &mut CompilerContext, item: &Item) -> Item {
        match &item.kind {
            ItemKind::Void => panic!("AssertionError: cannot load a void item"),
            ItemKind::Stack => item.clone(),
            ItemKind::Indexed => {
                self.code
                    .borrow_mut()
                    .emitop0(ctx, IALOAD + item.typecode);
                self.stack_item[item.typecode as usize].clone()
            }
            ItemKind::SelfItem { .. } => {
                self.code.borrow_mut().emitop0(ctx, ALOAD_0);
                self.stack_item[item.typecode as usize].clone()
            }
            ItemKind::Local { reg, .. } => {
                let truncated = Code::truncate(item.typecode);
                let mut code = self.code.borrow_mut();
                if *reg <= 3 {
                    code.emitop0(ctx, ILOAD_0 + truncated * 4 + *reg);
                } else {
                    code.emitop1w(ctx, ILOAD + truncated, *reg);
                }
                drop(code);
                self.stack_item[item.typecode as usize].clone()
            }
            ItemKind::Static { member } => {
                let syms = Rc::clone(&self.syms);
                let od = self
                    .pool_writer
                    .borrow_mut()
                    .put_member(ctx, &syms.borrow(), *member);
                self.code
                    .borrow_mut()
                    .emitop2_symbol(ctx, GETSTATIC, od, *member);
                self.stack_item[item.typecode as usize].clone()
            }
            ItemKind::Dynamic { member } => {
                assert!(
                    ctx.sym(*member).kind == crate::code::kinds::Kind::VAR,
                    "AssertionError: dynamic constant expected"
                );
                let ty = ctx.sym(*member).ty.expect("member type");
                let types = Rc::clone(&self.types);
                let syms = Rc::clone(&self.syms);
                let erased = types.erasure(ctx, &syms.borrow(), ty);
                let rescode = Code::typecode(ctx, erased);
                self.code
                    .borrow_mut()
                    .emit_ldc(ctx, LoadableConstantRef::Dynamic(*member));
                self.stack_item[rescode as usize].clone()
            }
            ItemKind::Member { member, .. } => {
                let syms = Rc::clone(&self.syms);
                let od = self
                    .pool_writer
                    .borrow_mut()
                    .put_member(ctx, &syms.borrow(), *member);
                self.code
                    .borrow_mut()
                    .emitop2_symbol(ctx, GETFIELD, od, *member);
                self.stack_item[item.typecode as usize].clone()
            }
            ItemKind::Immediate { value } => {
                self.load_immediate(ctx, item, value);
                self.stack_item[item.typecode as usize].clone()
            }
            ItemKind::Assign { lhs } => {
                let typecode = item.typecode;
                self.stash(ctx, lhs, typecode);
                self.store(ctx, lhs);
                self.stack_item[typecode as usize].clone()
            }
            ItemKind::Cond { .. } => self.load_cond(ctx, item),
        }
    }

    /// Generate code to load an immediate item.
    fn load_immediate(
        &mut self,
        ctx: &mut CompilerContext,
        item: &Item,
        value: &LoadableConstantRef,
    ) {
        let typecode = item.typecode;
        match typecode {
            INT_CODE | BYTE_CODE | SHORT_CODE | CHAR_CODE => {
                let ival = immediate_int(value);
                if (-1..=5).contains(&ival) {
                    self.code.borrow_mut().emitop0(ctx, ICONST_0 + ival);
                } else if (i8::MIN as i32..=i8::MAX as i32).contains(&ival) {
                    self.code.borrow_mut().emitop1(ctx, BIPUSH, ival);
                } else if (i16::MIN as i32..=i16::MAX as i32).contains(&ival) {
                    self.code.borrow_mut().emitop2(ctx, SIPUSH, ival);
                } else {
                    self.ldc(ctx, value);
                }
            }
            LONG_CODE => {
                let lval = immediate_long(value);
                if lval == 0 || lval == 1 {
                    self.code
                        .borrow_mut()
                        .emitop0(ctx, LCONST_0 + lval as i32);
                } else {
                    self.ldc(ctx, value);
                }
            }
            FLOAT_CODE => {
                let fval = immediate_float(value);
                if is_pos_zero_f32(fval) || fval == 1.0 || fval == 2.0 {
                    self.code
                        .borrow_mut()
                        .emitop0(ctx, FCONST_0 + fval as i32);
                } else {
                    self.ldc(ctx, value);
                }
            }
            DOUBLE_CODE => {
                let dval = immediate_double(value);
                if is_pos_zero_f64(dval) || dval == 1.0 {
                    self.code
                        .borrow_mut()
                        .emitop0(ctx, DCONST_0 + dval as i32);
                } else {
                    self.ldc(ctx, value);
                }
            }
            OBJECT_CODE => self.ldc(ctx, value),
            _ => panic!("AssertionError: unsupported immediate typecode"),
        }
    }

    /// Emit an ldc (or ldc2w) for a loadable constant.
    fn ldc(&mut self, ctx: &mut CompilerContext, value: &LoadableConstantRef) {
        if is_long_or_double(value) {
            let syms = Rc::clone(&self.syms);
            let od = self.pool_writer.borrow_mut().put_constant(
                ctx,
                &syms.borrow(),
                loadable_to_pool_constant(value),
            );
            self.code
                .borrow_mut()
                .emitop2_data(ctx, LDC2W, od, Some(value.clone()));
        } else {
            self.code.borrow_mut().emit_ldc(ctx, value.clone());
        }
    }

    /// Generate code to load a conditional item.
    fn load_cond(&mut self, ctx: &mut CompilerContext, item: &Item) -> Item {
        let (opcode, true_jumps, _false_jumps, _tree) = cond_parts(item);
        let mut true_chain = None;
        let false_chain = self.jump_false(ctx, item);
        if !(true_jumps.is_none() && opcode == JSR) {
            self.code
                .borrow_mut()
                .resolve_pending_chain(ctx, true_jumps.clone());
            self.code.borrow_mut().emitop0(ctx, ICONST_1);
            true_chain = self.code.borrow_mut().branch(ctx, GOTO);
        }
        if false_chain.is_some() {
            self.code
                .borrow_mut()
                .resolve_pending_chain(ctx, false_chain.clone());
            self.code.borrow_mut().emitop0(ctx, ICONST_0);
        }
        self.code
            .borrow_mut()
            .resolve_pending_chain(ctx, true_chain);
        self.stack_item[item.typecode as usize].clone()
    }

    /// Generate code to store the top of the stack into an item.
    pub fn store(&mut self, ctx: &mut CompilerContext, item: &Item) {
        match &item.kind {
            ItemKind::Indexed => {
                self.code.borrow_mut().emitop0(ctx, IASTORE + item.typecode);
            }
            ItemKind::Local { reg, .. } => {
                let truncated = Code::truncate(item.typecode);
                {
                    let mut code = self.code.borrow_mut();
                    if *reg <= 3 {
                        code.emitop0(ctx, ISTORE_0 + truncated * 4 + *reg);
                    } else {
                        code.emitop1w(ctx, ISTORE + truncated, *reg);
                    }
                }
                self.code.borrow_mut().set_defined_at(ctx, *reg);
            }
            ItemKind::Static { member } => {
                let syms = Rc::clone(&self.syms);
                let od = self
                    .pool_writer
                    .borrow_mut()
                    .put_member(ctx, &syms.borrow(), *member);
                self.code
                    .borrow_mut()
                    .emitop2_symbol(ctx, PUTSTATIC, od, *member);
            }
            ItemKind::Dynamic { .. } => {
                panic!("AssertionError: this method should not be invoked")
            }
            ItemKind::Member { member, .. } => {
                let syms = Rc::clone(&self.syms);
                let od = self
                    .pool_writer
                    .borrow_mut()
                    .put_member(ctx, &syms.borrow(), *member);
                self.code
                    .borrow_mut()
                    .emitop2_symbol(ctx, PUTFIELD, od, *member);
            }
            ItemKind::Assign { lhs } => {
                self.stash(ctx, lhs, item.typecode);
                self.store(ctx, lhs);
            }
            _ => panic!(
                "AssertionError: store unsupported: {}",
                self.describe(ctx, item)
            ),
        }
    }

    /// Generate code to invoke the method represented by an item.
    pub fn invoke(&mut self, ctx: &mut CompilerContext, item: &Item) -> Item {
        match &item.kind {
            ItemKind::Static { member } => {
                let ty = ctx.sym(*member).ty.expect("member type");
                let types = Rc::clone(&self.types);
                let syms = Rc::clone(&self.syms);
                let mtype = types.erasure(ctx, &syms.borrow(), ty);
                let restype = types.get_return_type_public(ctx, mtype);
                let rescode = Code::typecode(ctx, restype);
                self.code
                    .borrow_mut()
                    .emit_invokestatic(ctx, *member, mtype);
                self.stack_item[rescode as usize].clone()
            }
            ItemKind::Dynamic { member } => {
                assert!(
                    ctx.sym(*member).kind == crate::code::kinds::Kind::MTH,
                    "AssertionError: dynamic method expected"
                );
                let ty = ctx.sym(*member).ty.expect("member type");
                let types = Rc::clone(&self.types);
                let syms = Rc::clone(&self.syms);
                let mtype = types.erasure(ctx, &syms.borrow(), ty);
                let restype = types.get_return_type_public(ctx, mtype);
                let rescode = Code::typecode(ctx, restype);
                self.code
                    .borrow_mut()
                    .emit_invokedynamic(ctx, *member, mtype);
                self.stack_item[rescode as usize].clone()
            }
            ItemKind::Member {
                member, nonvirtual, ..
            } => {
                let types = Rc::clone(&self.types);
                let syms = Rc::clone(&self.syms);
                let names = Rc::clone(&self.pool_writer.borrow().names);
                let mtype =
                    crate::code::symbol::external_type(ctx, &types, &syms.borrow(), &names, *member);
                let restype = types.get_return_type_public(ctx, mtype);
                let rescode = Code::typecode(ctx, restype);
                let is_interface = {
                    let owner = ctx.sym(*member).owner.expect("member owner");
                    (ctx.sym(owner).flags_field & crate::code::flags::INTERFACE as i64) != 0
                };
                let mut code = self.code.borrow_mut();
                if is_interface && !*nonvirtual {
                    code.emit_invokeinterface(ctx, *member, mtype);
                } else if *nonvirtual {
                    code.emit_invokespecial(ctx, *member, mtype);
                } else {
                    code.emit_invokevirtual(ctx, *member, mtype);
                }
                drop(code);
                self.stack_item[rescode as usize].clone()
            }
            _ => panic!("AssertionError: {}", self.describe(ctx, item)),
        }
    }

    /// Generate code to use an item twice.
    pub fn duplicate(&mut self, ctx: &mut CompilerContext, item: &Item) {
        match &item.kind {
            ItemKind::Stack => {
                let op = if Code::width(item.typecode) == 2 {
                    DUP2
                } else {
                    DUP
                };
                self.code.borrow_mut().emitop0(ctx, op);
            }
            ItemKind::Indexed => {
                self.code.borrow_mut().emitop0(ctx, DUP2);
            }
            ItemKind::Member { .. } => {
                let object = self.stack_item[OBJECT_CODE as usize].clone();
                self.duplicate(ctx, &object);
            }
            ItemKind::Assign { .. } | ItemKind::Cond { .. } => {
                let loaded = self.load(ctx, item);
                self.duplicate(ctx, &loaded);
            }
            _ => {}
        }
    }

    /// Generate code to drop an item.
    pub fn drop_item(&mut self, ctx: &mut CompilerContext, item: &Item) {
        match &item.kind {
            ItemKind::Stack => {
                let op = if Code::width(item.typecode) == 2 {
                    POP2
                } else {
                    POP
                };
                self.code.borrow_mut().emitop0(ctx, op);
            }
            ItemKind::Indexed => {
                self.code.borrow_mut().emitop0(ctx, POP2);
            }
            ItemKind::Member { .. } => {
                let object = self.stack_item[OBJECT_CODE as usize].clone();
                self.drop_item(ctx, &object);
            }
            ItemKind::Assign { lhs } => {
                self.store(ctx, lhs);
            }
            ItemKind::Cond { .. } => {
                let loaded = self.load(ctx, item);
                self.drop_item(ctx, &loaded);
            }
            _ => {}
        }
    }

    /// Generate code to stash a copy of the top of the stack under an item.
    pub fn stash(&mut self, ctx: &mut CompilerContext, item: &Item, toscode: i32) {
        match &item.kind {
            ItemKind::Stack => {
                let op = (if Code::width(item.typecode) == 2 {
                    DUP_X2
                } else {
                    DUP_X1
                }) + 3 * (Code::width(toscode) - 1);
                self.code.borrow_mut().emitop0(ctx, op);
            }
            ItemKind::Indexed => {
                let op = DUP_X2 + 3 * (Code::width(toscode) - 1);
                self.code.borrow_mut().emitop0(ctx, op);
            }
            ItemKind::Member { .. } => {
                let object = self.stack_item[OBJECT_CODE as usize].clone();
                self.stash(ctx, &object, toscode);
            }
            ItemKind::Assign { .. } | ItemKind::Cond { .. } => {
                panic!(
                    "AssertionError: stash unsupported: {}",
                    self.describe(ctx, item)
                );
            }
            _ => {
                let stack = self.stack_item[toscode as usize].clone();
                self.duplicate(ctx, &stack);
            }
        }
    }

    /// Generate code to turn an item into a testable condition.
    pub fn mk_cond(&mut self, ctx: &mut CompilerContext, item: &Item) -> Item {
        match &item.kind {
            ItemKind::Immediate { value } => {
                let ival = immediate_int(value);
                let opcode = if ival != 0 { GOTO } else { JSR };
                self.make_cond_item(opcode, None, None)
            }
            ItemKind::Cond { .. } => item.clone(),
            _ => {
                self.load(ctx, item);
                self.make_cond_item(IFNE, None, None)
            }
        }
    }

    /// Generate code to coerce an item to the given type code.
    pub fn coerce(&mut self, ctx: &mut CompilerContext, item: &Item, targetcode: i32) -> Item {
        if item.typecode == targetcode {
            return item.clone();
        }
        if let ItemKind::Immediate { value } = &item.kind
            && self.coerce_immediate_possible(ctx, item, targetcode)
        {
            return self.make_coerced_immediate(ctx, item, value, targetcode);
        }
        self.load(ctx, item);
        let typecode1 = Code::truncate(item.typecode);
        let targetcode1 = Code::truncate(targetcode);
        if typecode1 != targetcode1 {
            let offset = if targetcode1 > typecode1 {
                targetcode1 - 1
            } else {
                targetcode1
            };
            self.code
                .borrow_mut()
                .emitop0(ctx, I2L + typecode1 * 3 + offset);
        }
        if targetcode != targetcode1 {
            self.code
                .borrow_mut()
                .emitop0(ctx, INT2BYTE + targetcode - BYTE_CODE);
        }
        self.stack_item[targetcode as usize].clone()
    }

    /// Return whether an immediate item can be coerced to the target code
    /// without emitting conversion code.
    fn coerce_immediate_possible(
        &self,
        ctx: &CompilerContext,
        item: &Item,
        targetcode: i32,
    ) -> bool {
        let targetcode1 = Code::truncate(targetcode);
        match targetcode {
            INT_CODE | LONG_CODE | FLOAT_CODE | DOUBLE_CODE | BYTE_CODE | CHAR_CODE
            | SHORT_CODE => true,
            _ => {
                let _ = (ctx, item, targetcode1);
                false
            }
        }
    }

    /// Build the coerced immediate item for a primitive target type code.
    fn make_coerced_immediate(
        &self,
        ctx: &CompilerContext,
        item: &Item,
        value: &LoadableConstantRef,
        targetcode: i32,
    ) -> Item {
        if item.typecode == targetcode {
            return item.clone();
        }
        match targetcode {
            INT_CODE => {
                if Code::truncate(item.typecode) == INT_CODE {
                    item.clone()
                } else {
                    self.make_immediate_item(
                        ctx,
                        self.syms.borrow().int_type,
                        ImmediateValue::Int(immediate_int(value)),
                    )
                }
            }
            LONG_CODE => self.make_immediate_item(
                ctx,
                self.syms.borrow().long_type,
                ImmediateValue::Long(immediate_long(value)),
            ),
            FLOAT_CODE => self.make_immediate_item(
                ctx,
                self.syms.borrow().float_type,
                ImmediateValue::Float(immediate_float(value)),
            ),
            DOUBLE_CODE => self.make_immediate_item(
                ctx,
                self.syms.borrow().double_type,
                ImmediateValue::Double(immediate_double(value)),
            ),
            BYTE_CODE => self.make_immediate_item(
                ctx,
                self.syms.borrow().byte_type,
                ImmediateValue::Int(immediate_int(value) as i8 as i32),
            ),
            CHAR_CODE => self.make_immediate_item(
                ctx,
                self.syms.borrow().char_type,
                ImmediateValue::Int(immediate_int(value) as u16 as i32),
            ),
            SHORT_CODE => self.make_immediate_item(
                ctx,
                self.syms.borrow().short_type,
                ImmediateValue::Int(immediate_int(value) as i16 as i32),
            ),
            _ => panic!("AssertionError: unsupported coercion target"),
        }
    }

    /// Make a conditional item with the given opcode and no jump chains.
    pub fn make_cond_item_opcode(&self, opcode: i32) -> Item {
        self.make_cond_item(opcode, None, None)
    }

    /// Coerce an item to the given target type.
    pub fn coerce_type(
        &mut self,
        ctx: &mut CompilerContext,
        item: &Item,
        targettype: TypeId,
    ) -> Item {
        let targetcode = Code::typecode(ctx, targettype);
        self.coerce(ctx, item, targetcode)
    }

    /// Return the width of an item on the stack in words.
    pub fn width(&self, item: &Item) -> i32 {
        match &item.kind {
            ItemKind::Void => 0,
            ItemKind::Stack => Code::width(item.typecode),
            ItemKind::Indexed => 2,
            ItemKind::Assign { lhs } => self.width(lhs) + Code::width(item.typecode),
            ItemKind::Member { .. } => 1,
            ItemKind::Cond { .. } => panic!("AssertionError: a CondItem has no width"),
            _ => 0,
        }
    }

    /// Generate code to increment a local variable.
    pub fn incr(&mut self, ctx: &mut CompilerContext, item: &Item, x: i32) {
        let reg = match &item.kind {
            ItemKind::Local { reg, .. } => *reg,
            _ => panic!("AssertionError: local item expected"),
        };
        if item.typecode == INT_CODE && (-32768..=32767).contains(&x) {
            self.code.borrow_mut().emitop1w_char(ctx, IINC, reg, x);
        } else {
            self.load(ctx, item);
            let literal = self.make_immediate_item(
                ctx,
                self.syms.borrow().int_type,
                ImmediateValue::Int(if x >= 0 { x } else { -x }),
            );
            self.load(ctx, &literal);
            self.code
                .borrow_mut()
                .emitop0(ctx, if x >= 0 { IADD } else { ISUB });
            let stack_int = self.make_stack_item(ctx, self.syms.borrow().int_type);
            let targetcode = item.typecode;
            self.coerce(ctx, &stack_int, targetcode);
            self.store(ctx, item);
        }
    }

    /// Generate code to branch when a conditional item is true.
    pub fn jump_true(&mut self, ctx: &mut CompilerContext, item: &Item) -> Option<Box<Chain>> {
        let (opcode, true_jumps, _, tree) = cond_parts(item);
        if tree.is_none() {
            let branch = self.code.borrow_mut().branch(ctx, opcode);
            return Code::merge_chains(true_jumps, branch);
        }
        let start_pc = self.code.borrow_mut().cur_cp(ctx);
        let branch = self.code.borrow_mut().branch(ctx, opcode);
        let merged = Code::merge_chains(true_jumps, branch);
        let end_pc = self.code.borrow_mut().cur_cp(ctx);
        if let Some(crt) = &mut self.code.borrow_mut().crt {
            crt.put(CrtKey::Tree(tree.expect("condition tree")), CRT_BRANCH_TRUE, start_pc, end_pc);
        }
        merged
    }

    /// Generate code to branch when a conditional item is false.
    pub fn jump_false(&mut self, ctx: &mut CompilerContext, item: &Item) -> Option<Box<Chain>> {
        let (opcode, _, false_jumps, tree) = cond_parts(item);
        let negated = Code::negate(opcode);
        if tree.is_none() {
            let branch = self.code.borrow_mut().branch(ctx, negated);
            return Code::merge_chains(false_jumps, branch);
        }
        let start_pc = self.code.borrow_mut().cur_cp(ctx);
        let branch = self.code.borrow_mut().branch(ctx, negated);
        let merged = Code::merge_chains(false_jumps, branch);
        let end_pc = self.code.borrow_mut().cur_cp(ctx);
        if let Some(crt) = &mut self.code.borrow_mut().crt {
            crt.put(CrtKey::Tree(tree.expect("condition tree")), CRT_BRANCH_FALSE, start_pc, end_pc);
        }
        merged
    }

    /// Return the negated conditional item.
    pub fn negate(&self, item: &Item) -> Item {
        let (opcode, true_jumps, false_jumps, tree) = cond_parts(item);
        Item::new(
            BYTE_CODE,
            ItemKind::Cond {
                opcode: Code::negate(opcode),
                true_jumps: false_jumps,
                false_jumps: true_jumps,
                tree,
            },
        )
    }

    /// Return whether a conditional item is always true.
    pub fn cond_is_true(&self, item: &Item) -> bool {
        let (opcode, _, false_jumps, _) = cond_parts(item);
        false_jumps.is_none() && opcode == GOTO
    }

    /// Return whether a conditional item is always false.
    pub fn cond_is_false(&self, item: &Item) -> bool {
        let (opcode, true_jumps, _, _) = cond_parts(item);
        true_jumps.is_none() && opcode == JSR
    }

    /// Return a textual description of an item.
    pub fn describe(&self, ctx: &CompilerContext, item: &Item) -> String {
        match &item.kind {
            ItemKind::Void => "void".to_string(),
            ItemKind::Stack => format!(
                "stack({})",
                TYPE_CODE_NAMES[item.typecode as usize]
            ),
            ItemKind::Indexed => format!("indexed({})", TYPE_CODE_NAMES[item.typecode as usize]),
            ItemKind::SelfItem { is_super } => {
                if *is_super { "super" } else { "this" }.to_string()
            }
            ItemKind::Local { reg, type_ } => {
                format!("localItem(type={:?}; reg={})", type_, reg)
            }
            ItemKind::Static { member } => {
                format!("static({})", symbol_name(ctx, *member))
            }
            ItemKind::Dynamic { member } => {
                format!("dynamic({})", symbol_name(ctx, *member))
            }
            ItemKind::Member {
                member, nonvirtual, ..
            } => {
                if *nonvirtual {
                    format!("member({} nonvirtual)", symbol_name(ctx, *member))
                } else {
                    format!("member({})", symbol_name(ctx, *member))
                }
            }
            ItemKind::Immediate { .. } => "immediate".to_string(),
            ItemKind::Assign { lhs } => format!("assign(lhs = {})", self.describe(ctx, lhs)),
            ItemKind::Cond { opcode, .. } => format!("cond({})", Code::mnem(*opcode)),
        }
    }
}

/// The extracted parts of a conditional item.
type CondParts = (i32, Option<Box<Chain>>, Option<Box<Chain>>, Option<TreeId>);

/// Return the parts of a conditional item.
fn cond_parts(item: &Item) -> CondParts {
    match &item.kind {
        ItemKind::Cond {
            opcode,
            true_jumps,
            false_jumps,
            tree,
        } => (*opcode, true_jumps.clone(), false_jumps.clone(), *tree),
        _ => panic!("AssertionError: conditional item expected"),
    }
}

/// Return the name of a symbol for item descriptions.
fn symbol_name(ctx: &CompilerContext, sym: SymbolId) -> String {
    ctx.sym(sym).name.to_java_string().to_string_lossy()
}

/// Extract an int value from a loadable constant.
fn immediate_int(value: &LoadableConstantRef) -> i32 {
    match basic_data(value) {
        PoolKey::Integer(v) => *v,
        PoolKey::Long(v) => *v as i32,
        PoolKey::Float(bits) => f32::from_bits(*bits) as i32,
        PoolKey::Double(bits) => f64::from_bits(*bits) as i32,
        _ => panic!("AssertionError: numeric literal expected"),
    }
}

/// Extract a long value from a loadable constant.
fn immediate_long(value: &LoadableConstantRef) -> i64 {
    match basic_data(value) {
        PoolKey::Integer(v) => *v as i64,
        PoolKey::Long(v) => *v,
        PoolKey::Float(bits) => f32::from_bits(*bits) as i64,
        PoolKey::Double(bits) => f64::from_bits(*bits) as i64,
        _ => panic!("AssertionError: numeric literal expected"),
    }
}

/// Extract a float value from a loadable constant.
fn immediate_float(value: &LoadableConstantRef) -> f32 {
    match basic_data(value) {
        PoolKey::Integer(v) => *v as f32,
        PoolKey::Long(v) => *v as f32,
        PoolKey::Float(bits) => f32::from_bits(*bits),
        PoolKey::Double(bits) => f64::from_bits(*bits) as f32,
        _ => panic!("AssertionError: numeric literal expected"),
    }
}

/// Extract a double value from a loadable constant.
fn immediate_double(value: &LoadableConstantRef) -> f64 {
    match basic_data(value) {
        PoolKey::Integer(v) => *v as f64,
        PoolKey::Long(v) => *v as f64,
        PoolKey::Float(bits) => f64::from(f32::from_bits(*bits)),
        PoolKey::Double(bits) => f64::from_bits(*bits),
        _ => panic!("AssertionError: numeric literal expected"),
    }
}

/// Return whether a loadable constant is a long or double constant.
fn is_long_or_double(value: &LoadableConstantRef) -> bool {
    match value {
        LoadableConstantRef::Basic(basic) => {
            basic.tag == crate::jvm::class_file::CONSTANT_LONG
                || basic.tag == crate::jvm::class_file::CONSTANT_DOUBLE
        }
        _ => false,
    }
}

/// Return the constant data of a basic loadable constant.
fn basic_data(value: &LoadableConstantRef) -> &PoolKey {
    match value {
        LoadableConstantRef::Basic(basic) => &basic.data,
        _ => panic!("AssertionError: basic constant expected"),
    }
}

/// Return whether a float is positive zero.
fn is_pos_zero_f32(x: f32) -> bool {
    x == 0.0f32 && 1.0f32 / x > 0.0f32
}

/// Return whether a double is positive zero.
fn is_pos_zero_f64(x: f64) -> bool {
    x == 0.0f64 && 1.0f64 / x > 0.0f64
}

impl Items {
    /// Return the opcode of a conditional item.
    pub fn cond_opcode(&self, item: &Item) -> i32 {
        cond_parts(item).0
    }

    /// Return the true-jump chain of a conditional item.
    pub fn cond_true_jumps(&self, item: &Item) -> Option<Box<Chain>> {
        cond_parts(item).1
    }

    /// Return the false-jump chain of a conditional item.
    pub fn cond_false_jumps(&self, item: &Item) -> Option<Box<Chain>> {
        cond_parts(item).2
    }

    /// Set the condition tree of a conditional item.
    pub fn set_cond_tree(&self, item: &mut Item, tree: TreeId) {
        if let ItemKind::Cond { tree: slot, .. } = &mut item.kind {
            *slot = Some(tree);
        } else {
            panic!("AssertionError: conditional item expected");
        }
    }

    /// Make a conditional item with an explicit condition tree.
    pub fn make_cond_item_with_tree(
        &self,
        opcode: i32,
        true_jumps: Option<Box<Chain>>,
        false_jumps: Option<Box<Chain>>,
        tree: Option<TreeId>,
    ) -> Item {
        Item::new(
            BYTE_CODE,
            ItemKind::Cond {
                opcode,
                true_jumps,
                false_jumps,
                tree,
            },
        )
    }
}
