// Source: com/sun/tools/javac/jvm/Code.java

//! An internal structure corresponding to the code attribute of methods in a
//! class file, plus the bytecode emission utilities.

use std::cell::RefCell;
use std::rc::Rc;

use crate::code::symtab::Symtab;
use crate::code::type_tag::TypeTag;
use crate::code::types::Types;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TypeId};
use crate::jvm::byte_codes::*;
use crate::jvm::class_file;
use crate::jvm::cr_table::CRTable;
use crate::jvm::pool_constant::LoadableConstantRef;
use crate::jvm::pool_writer::PoolWriter;
use crate::jvm::stack_map::{StackMapFormat, StackMapFrame, StackMapTableFrame};
use crate::resources::compiler_properties::errors;
use crate::util::array_utils;
use crate::util::bits::Bits;
use crate::util::jc_diagnostic::Error;
use crate::util::list::List;
use crate::util::log::Log;
use crate::util::position::{LineMap, NOPOS};

pub use crate::jvm::code_state::{Chain, LocalVar, Range, State};

/// The code attribute of one method.
pub struct Code {
    /// Whether bytecode debug output is enabled.
    pub debug_code: bool,
    /// Whether stack map frames are required.
    pub need_stack_map: bool,
    /// The maximum operand stack size.
    pub max_stack: i32,
    /// The maximum number of local variable slots.
    pub max_locals: i32,
    /// The code buffer.
    pub code: Vec<u8>,
    /// The current code pointer.
    pub cp: i32,
    /// The expression catch data buffer; each entry is
    /// [start_pc, end_pc, handler_pc, catch_type].
    pub catch_info: Vec<[u16; 4]>,
    /// The line number info, handled in stack fashion.
    pub line_info: List<[u16; 2]>,
    /// The CharacterRangeTable of the method, if any.
    pub crt: Option<CRTable>,
    /// Whether jumps may be >= 32K.
    pub fatcode: bool,
    /// Whether code generation is enabled.
    pub alive: bool,
    /// The current machine state.
    pub state: State,
    /// Whether compactification is forbidden (something points to cp).
    pub fixed_pc: bool,
    /// The next available register.
    pub nextreg: i32,
    /// The chain of jumps to be resolved before the next opcode.
    pub pending_jumps: Option<Box<Chain>>,
    /// The position of the current statement, or NOPOS.
    pub pending_stat_pos: i32,
    /// Set when a stack map is needed at the current PC.
    pub pending_stack_map: bool,
    /// The stack map format.
    pub stack_map: StackMapFormat,
    /// Whether variable debug info is emitted.
    pub var_debug_info: bool,
    /// Whether line number info is emitted.
    pub line_debug_info: bool,
    /// The source line map, if any.
    pub line_map: Option<Rc<dyn LineMap>>,
    /// The method symbol this code belongs to.
    pub meth: SymbolId,
    /// The stack position used for let expressions.
    pub let_expr_stack_pos: i32,
    /// The CLDC stack map buffer.
    pub stack_map_buffer: Vec<StackMapFrame>,
    /// The compressed StackMapTable entry buffer.
    pub stack_map_table_buffer: Vec<StackMapTableFrame>,
    /// The number of used entries in the stack map buffers.
    pub stack_map_buffer_size: i32,
    /// The last PC at which a stack map was generated.
    pub last_stack_map_pc: i32,
    /// The last stack map frame.
    pub last_frame: Option<StackMapFrame>,
    /// The stack map frame before the last one.
    pub frame_before_last: Option<StackMapFrame>,
    /// The local variables, indexed by register.
    pub lvar: Vec<Option<LocalVar>>,
    /// The previously live local variables to output.
    pub var_buffer: Vec<Option<LocalVar>>,
    /// The number of used entries in var_buffer.
    pub var_buffer_size: i32,
    /// Type relations.
    pub types: Rc<Types>,
    /// The symbol table.
    pub syms: Rc<Symtab>,
    /// The constant pool writer.
    pub pool_writer: Rc<RefCell<PoolWriter>>,
}

impl Code {
    /// Create a code object for a method.
    ///
    /// # Arguments
    ///   - meth: the method symbol;
    ///   - fatcode: whether jump targets may exceed the short range;
    ///   - line_map: the source line map (None disables line debug info);
    ///   - var_debug_info: whether local variable debug info is emitted;
    ///   - stack_map: the stack map format;
    ///   - debug_code: whether bytecode debug output is enabled;
    ///   - crt: the character range table, if -Xjcov is enabled;
    ///   - syms: the symbol table;
    ///   - types: the type relations;
    ///   - pool_writer: the constant pool writer.
    ///
    /// # Returns
    ///   - the initialized code buffer.
    ///
    /// # Panics
    ///   - none.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        meth: SymbolId,
        fatcode: bool,
        line_map: Option<Rc<dyn LineMap>>,
        var_debug_info: bool,
        stack_map: StackMapFormat,
        debug_code: bool,
        crt: Option<CRTable>,
        syms: Rc<Symtab>,
        types: Rc<Types>,
        pool_writer: Rc<RefCell<PoolWriter>>,
    ) -> Code {
        let line_debug_info = line_map.is_some();
        let need_stack_map = matches!(stack_map, StackMapFormat::CLDC | StackMapFormat::JSR202);
        Code {
            debug_code,
            need_stack_map,
            max_stack: 0,
            max_locals: 0,
            code: vec![0u8; 64],
            cp: 0,
            catch_info: Vec::new(),
            line_info: List::nil(),
            crt,
            fatcode,
            alive: true,
            state: State::new(),
            fixed_pc: false,
            nextreg: 0,
            pending_jumps: None,
            pending_stat_pos: NOPOS,
            pending_stack_map: false,
            stack_map,
            var_debug_info,
            line_debug_info,
            line_map,
            meth,
            let_expr_stack_pos: 0,
            stack_map_buffer: Vec::new(),
            stack_map_table_buffer: Vec::new(),
            stack_map_buffer_size: 0,
            last_stack_map_pc: -1,
            last_frame: None,
            frame_before_last: None,
            lvar: vec![None; 20],
            var_buffer: Vec::new(),
            var_buffer_size: 0,
            types,
            syms,
            pool_writer,
        }
    }

    /// Return the type code of a type (JVM verifier narrowing).
    ///
    /// # Arguments
    ///   - type: the type id.
    ///
    /// # Returns
    ///   - the ByteCodes type code.
    ///
    /// # Panics
    ///   - none.
    pub fn typecode(ctx: &CompilerContext, type_: TypeId) -> i32 {
        match ctx.typ(type_).get_tag() {
            TypeTag::BYTE => BYTE_CODE,
            TypeTag::SHORT => SHORT_CODE,
            TypeTag::CHAR => CHAR_CODE,
            TypeTag::INT => INT_CODE,
            TypeTag::LONG => LONG_CODE,
            TypeTag::FLOAT => FLOAT_CODE,
            TypeTag::DOUBLE => DOUBLE_CODE,
            TypeTag::BOOLEAN => BYTE_CODE,
            TypeTag::VOID => VOID_CODE,
            TypeTag::CLASS
            | TypeTag::ARRAY
            | TypeTag::METHOD
            | TypeTag::BOT
            | TypeTag::TYPEVAR
            | TypeTag::UNINITIALIZED_THIS
            | TypeTag::UNINITIALIZED_OBJECT => OBJECT_CODE,
            tag => panic!("AssertionError: typecode {}", tag),
        }
    }

    /// Narrow a type code to its verifier type code.
    ///
    /// # Arguments
    ///   - tc: the type code.
    ///
    /// # Returns
    ///   - INT_CODE for byte/short/char, the code otherwise.
    ///
    /// # Panics
    ///   - none.
    pub fn truncate(tc: i32) -> i32 {
        match tc {
            BYTE_CODE | SHORT_CODE | CHAR_CODE => INT_CODE,
            _ => tc,
        }
    }

    /// Return the verifier width of a type code.
    ///
    /// # Arguments
    ///   - typecode: the type code.
    ///
    /// # Returns
    ///   - 2 for long/double, 0 for void, 1 otherwise.
    ///
    /// # Panics
    ///   - none.
    pub fn width(typecode: i32) -> i32 {
        match typecode {
            LONG_CODE | DOUBLE_CODE => 2,
            VOID_CODE => 0,
            _ => 1,
        }
    }

    /// Return the verifier width of a type.
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - syms: the symbol table;
    ///   - type: the type, None meaning the unknown type (width 1).
    ///
    /// # Returns
    ///   - the verifier width.
    ///
    /// # Panics
    ///   - none.
    pub fn width_type(ctx: &CompilerContext, _syms: &Symtab, type_: Option<TypeId>) -> i32 {
        match type_ {
            Some(t) => Code::width(Code::typecode(ctx, t)),
            None => 1,
        }
    }

    /// Return the total verifier width of a type list.
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - syms: the symbol table;
    ///   - types: the type list.
    ///
    /// # Returns
    ///   - the sum of the verifier widths.
    ///
    /// # Panics
    ///   - none.
    pub fn width_list(ctx: &CompilerContext, syms: &Symtab, types: &List<TypeId>) -> i32 {
        let mut w = 0;
        let mut current = types.clone();
        while current.non_empty() {
            w += Code::width(Code::typecode(ctx, *current.head()));
            current = current.tail();
        }
        let _ = syms;
        w
    }

    /// Return the array allocation code of a type.
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - type: the element type.
    ///
    /// # Returns
    ///   - the JVMS newarray atype code (0/1 for class/array).
    ///
    /// # Panics
    ///   - the type cannot be allocated as an array.
    pub fn arraycode(ctx: &CompilerContext, type_: TypeId) -> i32 {
        match ctx.typ(type_).get_tag() {
            TypeTag::BYTE => 8,
            TypeTag::BOOLEAN => 4,
            TypeTag::SHORT => 9,
            TypeTag::CHAR => 5,
            TypeTag::INT => 10,
            TypeTag::LONG => 11,
            TypeTag::FLOAT => 6,
            TypeTag::DOUBLE => 7,
            TypeTag::CLASS => 0,
            TypeTag::ARRAY => 1,
            tag => panic!("AssertionError: arraycode {}", tag),
        }
    }
}
impl Code {
    /// Return the current output code pointer, resolving pending jumps first.
    pub fn cur_cp(&mut self, ctx: &mut CompilerContext) -> i32 {
        if self.pending_jumps.is_some() {
            self.resolve_pending(ctx);
        }
        if self.pending_stat_pos != NOPOS {
            self.mark_stat_begin(ctx);
        }
        self.fixed_pc = true;
        self.cp
    }

    /// Emit one byte of code.
    fn emit1(&mut self, od: i32) {
        if !self.alive {
            return;
        }
        if self.code.len() < (self.cp + 1) as usize {
            let code = std::mem::take(&mut self.code);
            self.code = array_utils::ensure_capacity_bytes(code, self.cp);
        }
        self.code[self.cp as usize] = (od & 0xFF) as u8;
        self.cp += 1;
    }

    /// Emit two bytes of code.
    fn emit2(&mut self, od: i32) {
        if !self.alive {
            return;
        }
        if self.cp + 2 > self.code.len() as i32 {
            self.emit1(od >> 8);
            self.emit1(od);
        } else {
            self.code[self.cp as usize] = ((od >> 8) & 0xFF) as u8;
            self.code[(self.cp + 1) as usize] = (od & 0xFF) as u8;
            self.cp += 2;
        }
    }

    /// Emit four bytes of code.
    pub fn emit4(&mut self, od: i32) {
        if !self.alive {
            return;
        }
        if self.cp + 4 > self.code.len() as i32 {
            self.emit1(od >> 24);
            self.emit1(od >> 16);
            self.emit1(od >> 8);
            self.emit1(od);
        } else {
            self.code[self.cp as usize] = ((od >> 24) & 0xFF) as u8;
            self.code[(self.cp + 1) as usize] = ((od >> 16) & 0xFF) as u8;
            self.code[(self.cp + 2) as usize] = ((od >> 8) & 0xFF) as u8;
            self.code[(self.cp + 3) as usize] = (od & 0xFF) as u8;
            self.cp += 4;
        }
    }

    /// Emit an opcode.
    fn emitop(&mut self, ctx: &mut CompilerContext, op: i32) {
        if self.pending_jumps.is_some() {
            self.resolve_pending(ctx);
        }
        if self.alive {
            if self.pending_stat_pos != NOPOS {
                self.mark_stat_begin(ctx);
            }
            if self.pending_stack_map {
                self.pending_stack_map = false;
                self.emit_stack_map(ctx);
            }
            if self.debug_code {
                eprintln!(
                    "emit@{} stack={}: {}",
                    self.cp,
                    self.state.stacksize,
                    Code::mnem(op)
                );
            }
            self.emit1(op);
        }
    }

    /// Check that code generation ended in a valid state.
    fn postop(&self) {
        assert!(
            self.alive || self.is_statement_start(),
            "AssertionError: code generation state"
        );
    }

    /// Push a value and update the maximum stack size.
    fn push(&mut self, ctx: &mut CompilerContext, t: TypeId) {
        let syms_rc = Rc::clone(&self.syms);
        let syms = &*syms_rc;
        self.state.push(ctx, &syms, t);
        if self.state.stacksize > self.max_stack {
            self.max_stack = self.state.stacksize;
        }
    }

    /// Pop one single-width value.
    fn pop1(&mut self, ctx: &mut CompilerContext) -> TypeId {
        let syms_rc = Rc::clone(&self.syms);
        let syms = &*syms_rc;
        self.state.pop1(ctx, &syms)
    }

    /// Pop one two-slot value.
    fn pop2(&mut self, ctx: &mut CompilerContext) -> TypeId {
        let syms_rc = Rc::clone(&self.syms);
        let syms = &*syms_rc;
        self.state.pop2(ctx, &syms)
    }

    /// Pop a value of the given type.
    fn pop_type(&mut self, ctx: &mut CompilerContext, t: TypeId) {
        let syms_rc = Rc::clone(&self.syms);
        let syms = &*syms_rc;
        self.state.pop_type(ctx, &syms, t);
    }

    /// Pop n slots.
    fn pop_n(&mut self, n: i32) {
        self.state.pop(n);
    }

    /// Return the topmost stack type.
    fn peek(&self) -> TypeId {
        self.state.peek()
    }

    /// Replace an uninitialized type by its initialized type in the state and in
    /// the local variable table.
    fn mark_initialized(&mut self, ctx: &mut CompilerContext, old: TypeId) {
        let newtype = crate::jvm::uninitialized_type::initialized_type(ctx, old)
            .expect("AssertionError: expected an uninitialized type");
        for i in 0..self.state.stacksize {
            if self.state.stack[i as usize] == Some(old) {
                self.state.stack[i as usize] = Some(newtype);
            }
        }
        for i in 0..self.lvar.len() {
            let Some(lv) = self.lvar[i].clone() else {
                continue;
            };
            let Some(lv_ty) = ctx.sym(lv.sym).ty else {
                continue;
            };
            if lv_ty != old {
                continue;
            }
            let mut sym = ctx.sym(lv.sym).clone();
            sym.ty = Some(newtype);
            let new_sym = ctx.alloc_symbol(sym);
            let mut new_lv = LocalVar::new(new_sym, lv.reg);
            new_lv.alive_ranges = lv.alive_ranges.clone();
            self.lvar[i] = Some(new_lv);
        }
    }

    /// Emit a ldc (or ldc_w) instruction.
    pub fn emit_ldc(&mut self, ctx: &mut CompilerContext, constant: LoadableConstantRef) {
        let syms_rc = Rc::clone(&self.syms);
        let syms = &*syms_rc;
        let od = {
            let mut pool_writer = self.pool_writer.borrow_mut();
            pool_writer.put_constant(
                ctx,
                &syms,
                crate::jvm::pool_constant::loadable_to_pool_constant(&constant),
            )
        };
        if od <= 255 {
            self.emitop1_data(ctx, LDC1, od, Some(constant));
        } else {
            self.emitop2_data(ctx, LDC2, od, Some(constant));
        }
    }

    /// Emit a multianewarray instruction.
    pub fn emit_multianewarray(
        &mut self,
        ctx: &mut CompilerContext,
        ndims: i32,
        type_: i32,
        array_type: TypeId,
    ) {
        self.emitop(ctx, MULTIANEWARRAY);
        if !self.alive {
            return;
        }
        self.emit2(type_);
        self.emit1(ndims);
        self.pop_n(ndims);
        self.push(ctx, array_type);
    }

    /// Emit a newarray instruction.
    pub fn emit_newarray(&mut self, ctx: &mut CompilerContext, elemcode: i32, array_type: TypeId) {
        self.emitop(ctx, NEWARRAY);
        if !self.alive {
            return;
        }
        self.emit1(elemcode);
        self.pop_n(1);
        self.push(ctx, array_type);
    }

    /// Emit an anewarray instruction.
    pub fn emit_anewarray(&mut self, ctx: &mut CompilerContext, od: i32, array_type: TypeId) {
        self.emitop(ctx, ANEWARRAY);
        if !self.alive {
            return;
        }
        self.emit2(od);
        self.pop_n(1);
        self.push(ctx, array_type);
    }
}
impl Code {
    /// Return the total verifier width of a method type's parameter types.
    fn method_argsize(&mut self, ctx: &mut CompilerContext, mtype: TypeId) -> i32 {
        let params = self.types.get_parameter_types_public(ctx, mtype);
        let syms_rc = Rc::clone(&self.syms);
        let syms = &*syms_rc;
        Code::width_list(ctx, &syms, &params)
    }

    /// Return the return type of a method type.
    fn method_restype(&self, ctx: &mut CompilerContext, mtype: TypeId) -> TypeId {
        self.types.get_return_type_public(ctx, mtype)
    }

    /// Emit an invokeinterface instruction.
    pub fn emit_invokeinterface(
        &mut self,
        ctx: &mut CompilerContext,
        member: SymbolId,
        mtype: TypeId,
    ) {
        let argsize = self.method_argsize(ctx, mtype);
        self.emitop(ctx, INVOKEINTERFACE);
        if !self.alive {
            return;
        }
        let syms_rc = Rc::clone(&self.syms);
        let syms = &*syms_rc;
        let index = {
            let mut pool_writer = self.pool_writer.borrow_mut();
            pool_writer.put_member(ctx, &syms, member)
        };
        self.emit2(index);
        self.emit1(argsize + 1);
        self.emit1(0);
        self.pop_n(argsize + 1);
        let restype = self.method_restype(ctx, mtype);
        self.push(ctx, restype);
    }

    /// Emit an invokespecial instruction.
    pub fn emit_invokespecial(
        &mut self,
        ctx: &mut CompilerContext,
        member: SymbolId,
        mtype: TypeId,
    ) {
        let argsize = self.method_argsize(ctx, mtype);
        self.emitop(ctx, INVOKESPECIAL);
        if !self.alive {
            return;
        }
        let syms_rc = Rc::clone(&self.syms);
        let syms = &*syms_rc;
        let index = {
            let mut pool_writer = self.pool_writer.borrow_mut();
            pool_writer.put_member(ctx, &syms, member)
        };
        self.emit2(index);
        self.pop_n(argsize);
        if Code::symbol_is_constructor(ctx, member) {
            let old = self.peek();
            if !matches!(
                &ctx.typ(old).kind,
                crate::code::type_::TypeKindData::UninitializedType { .. }
            ) {}
            self.mark_initialized(ctx, old);
        }
        self.pop_n(1);
        let restype = self.method_restype(ctx, mtype);
        self.push(ctx, restype);
    }

    /// Return whether a symbol is a constructor.
    fn symbol_is_constructor(ctx: &CompilerContext, sym: SymbolId) -> bool {
        ctx.sym(sym).name.to_java_string().to_string_lossy() == "<init>"
    }

    /// Emit an invokestatic instruction.
    pub fn emit_invokestatic(
        &mut self,
        ctx: &mut CompilerContext,
        member: SymbolId,
        mtype: TypeId,
    ) {
        let argsize = self.method_argsize(ctx, mtype);
        self.emitop(ctx, INVOKESTATIC);
        if !self.alive {
            return;
        }
        let syms_rc = Rc::clone(&self.syms);
        let syms = &*syms_rc;
        let index = {
            let mut pool_writer = self.pool_writer.borrow_mut();
            pool_writer.put_member(ctx, &syms, member)
        };
        self.emit2(index);
        self.pop_n(argsize);
        let restype = self.method_restype(ctx, mtype);
        self.push(ctx, restype);
    }

    /// Emit an invokevirtual instruction.
    pub fn emit_invokevirtual(
        &mut self,
        ctx: &mut CompilerContext,
        member: SymbolId,
        mtype: TypeId,
    ) {
        let argsize = self.method_argsize(ctx, mtype);
        self.emitop(ctx, INVOKEVIRTUAL);
        if !self.alive {
            return;
        }
        let syms_rc = Rc::clone(&self.syms);
        let syms = &*syms_rc;
        let index = {
            let mut pool_writer = self.pool_writer.borrow_mut();
            pool_writer.put_member(ctx, &syms, member)
        };
        self.emit2(index);
        self.pop_n(argsize + 1);
        let restype = self.method_restype(ctx, mtype);
        self.push(ctx, restype);
    }

    /// Emit an invokedynamic instruction.
    pub fn emit_invokedynamic(
        &mut self,
        ctx: &mut CompilerContext,
        dyn_member: SymbolId,
        mtype: TypeId,
    ) {
        let argsize = self.method_argsize(ctx, mtype);
        self.emitop(ctx, INVOKEDYNAMIC);
        if !self.alive {
            return;
        }
        let syms_rc = Rc::clone(&self.syms);
        let syms = &*syms_rc;
        let index = {
            let mut pool_writer = self.pool_writer.borrow_mut();
            pool_writer.put_dynamic(ctx, &syms, dyn_member)
        };
        self.emit2(index);
        self.emit2(0);
        self.pop_n(argsize);
        let restype = self.method_restype(ctx, mtype);
        self.push(ctx, restype);
    }
}
impl Code {
    /// Emit an opcode with no operand field.
    pub fn emitop0(&mut self, ctx: &mut CompilerContext, op: i32) {
        self.emitop(ctx, op);
        if !self.alive {
            return;
        }
        match op {
            AALOAD => {
                self.pop_n(1); // index
                let a = self.state.stack[(self.state.stacksize - 1) as usize]
                    .expect("AssertionError: empty operand stack");
                assert!(
                    !ctx.typ(a).has_tag(TypeTag::BOT),
                    "AssertionError: null type cannot be indexed"
                );
                self.pop_n(1);
                let syms_rc = Rc::clone(&self.syms);
                let syms = &*syms_rc;
                let elem = self.types.elemtype(ctx, &syms, a);
                let erased = self.types.erasure(ctx, &syms, elem);
                self.push(ctx, erased);
            }
            GOTO => {
                self.mark_dead();
            }
            NOP | INEG | LNEG | FNEG | DNEG => {}
            ACONST_NULL => {
                let bot = self.syms.bot_type;
                self.push(ctx, bot);
            }
            ICONST_M1 | ICONST_0 | ICONST_1 | ICONST_2 | ICONST_3 | ICONST_4 | ICONST_5
            | ILOAD_0 | ILOAD_1 | ILOAD_2 | ILOAD_3 => {
                let int = self.syms.int_type;
                self.push(ctx, int);
            }
            LCONST_0 | LCONST_1 | LLOAD_0 | LLOAD_1 | LLOAD_2 | LLOAD_3 => {
                let long = self.syms.long_type;
                self.push(ctx, long);
            }
            FCONST_0 | FCONST_1 | FCONST_2 | FLOAD_0 | FLOAD_1 | FLOAD_2 | FLOAD_3 => {
                let float = self.syms.float_type;
                self.push(ctx, float);
            }
            DCONST_0 | DCONST_1 | DLOAD_0 | DLOAD_1 | DLOAD_2 | DLOAD_3 => {
                let double = self.syms.double_type;
                self.push(ctx, double);
            }
            ALOAD_0 => {
                let t = ctx
                    .sym(self.lvar[0].as_ref().expect("local 0").sym)
                    .ty
                    .expect("local type");
                self.push(ctx, t);
            }
            ALOAD_1 => {
                let Some(local) = self.lvar[1].as_ref() else {
                    let method = ctx.sym(self.meth);
                    let owner = method
                        .owner
                        .map(|owner| crate::comp::resolve::symbol_display_name(ctx, owner))
                        .unwrap_or_default();
                    let (p, ep, cl) = match &method.kind_data {
                        crate::code::symbol::SymbolKind::MethodSymbol {
                            params,
                            extra_params,
                            captured_locals,
                            ..
                        } => (
                            params.length(),
                            extra_params.length(),
                            captured_locals.length(),
                        ),
                        _ => (-1, -1, -1),
                    };
                    let locals: Vec<String> = self
                        .lvar
                        .iter()
                        .enumerate()
                        .filter_map(|(i, v)| {
                            v.as_ref().map(|v| {
                                format!(
                                    "{i}:{}@{:?}",
                                    ctx.sym(v.sym).name.to_java_string().to_string_lossy(),
                                    ctx.sym(v.sym).local_adr()
                                )
                            })
                        })
                        .collect();
                    panic!(
                        "ALOAD_1 without local 1 in {}::{} nextreg={} lvars={} params={p} extra={ep} captured={cl} locals={locals:?}",
                        owner,
                        method.name.to_java_string().to_string_lossy(),
                        self.nextreg,
                        self.lvar.len(),
                    );
                };
                let t = ctx.sym(local.sym).ty.expect("local type");
                self.push(ctx, t);
            }
            ALOAD_2 => {
                let t = ctx
                    .sym(self.lvar[2].as_ref().expect("local 2").sym)
                    .ty
                    .expect("local type");
                self.push(ctx, t);
            }
            ALOAD_3 => {
                let t = ctx
                    .sym(self.lvar[3].as_ref().expect("local 3").sym)
                    .ty
                    .expect("local type");
                self.push(ctx, t);
            }
            IALOAD | BALOAD | CALOAD | SALOAD => {
                self.pop_n(2);
                let int = self.syms.int_type;
                self.push(ctx, int);
            }
            LALOAD => {
                self.pop_n(2);
                let long = self.syms.long_type;
                self.push(ctx, long);
            }
            FALOAD => {
                self.pop_n(2);
                let float = self.syms.float_type;
                self.push(ctx, float);
            }
            DALOAD => {
                self.pop_n(2);
                let double = self.syms.double_type;
                self.push(ctx, double);
            }
            ISTORE_0 | ISTORE_1 | ISTORE_2 | ISTORE_3 | FSTORE_0 | FSTORE_1 | FSTORE_2
            | FSTORE_3 | ASTORE_0 | ASTORE_1 | ASTORE_2 | ASTORE_3 | POP | LSHR | LSHL | LUSHR => {
                self.pop_n(1);
            }
            ARETURN | IRETURN | FRETURN => {
                assert_eq!(self.state.nlocks, 0, "AssertionError: locks active on return");
                self.pop_n(1);
                self.mark_dead();
            }
            ATHROW => {
                self.pop_n(1);
                self.mark_dead();
            }
            LSTORE_0 | LSTORE_1 | LSTORE_2 | LSTORE_3 | DSTORE_0 | DSTORE_1 | DSTORE_2
            | DSTORE_3 | POP2 => {
                self.pop_n(2);
            }
            LRETURN | DRETURN => {
                assert_eq!(self.state.nlocks, 0, "AssertionError: locks active on return");
                self.pop_n(2);
                self.mark_dead();
            }
            DUP => {
                let t = self.state.stack[(self.state.stacksize - 1) as usize]
                    .expect("AssertionError: empty operand stack");
                self.push(ctx, t);
            }
            RETURN => {
                assert_eq!(self.state.nlocks, 0, "AssertionError: locks active on return");
                self.mark_dead();
            }
            ARRAYLENGTH => {
                self.pop_n(1);
                let int = self.syms.int_type;
                self.push(ctx, int);
            }
            ISUB | IADD | IMUL | IDIV | IMOD | ISHL | ISHR | IUSHR | IAND | IOR | IXOR => {
                self.pop_n(1);
            }
            AASTORE => {
                self.pop_n(3);
            }
            LAND | LOR | LXOR | LMOD | LDIV | LMUL | LSUB | LADD => {
                self.pop_n(2);
            }
            LCMP => {
                self.pop_n(4);
                let int = self.syms.int_type;
                self.push(ctx, int);
            }
            L2I => {
                self.pop_n(2);
                let int = self.syms.int_type;
                self.push(ctx, int);
            }
            I2L => {
                self.pop_n(1);
                let long = self.syms.long_type;
                self.push(ctx, long);
            }
            I2F => {
                self.pop_n(1);
                let float = self.syms.float_type;
                self.push(ctx, float);
            }
            I2D => {
                self.pop_n(1);
                let double = self.syms.double_type;
                self.push(ctx, double);
            }
            L2F => {
                self.pop_n(2);
                let float = self.syms.float_type;
                self.push(ctx, float);
            }
            L2D => {
                self.pop_n(2);
                let double = self.syms.double_type;
                self.push(ctx, double);
            }
            F2I => {
                self.pop_n(1);
                let int = self.syms.int_type;
                self.push(ctx, int);
            }
            F2L => {
                self.pop_n(1);
                let long = self.syms.long_type;
                self.push(ctx, long);
            }
            F2D => {
                self.pop_n(1);
                let double = self.syms.double_type;
                self.push(ctx, double);
            }
            D2I => {
                self.pop_n(2);
                let int = self.syms.int_type;
                self.push(ctx, int);
            }
            D2L => {
                self.pop_n(2);
                let long = self.syms.long_type;
                self.push(ctx, long);
            }
            D2F => {
                self.pop_n(2);
                let float = self.syms.float_type;
                self.push(ctx, float);
            }
            TABLESWITCH | LOOKUPSWITCH => {
                self.pop_n(1);
            }
            DUP_X1 => {
                let val1 = self.pop1(ctx);
                let val2 = self.pop1(ctx);
                self.push(ctx, val1);
                self.push(ctx, val2);
                self.push(ctx, val1);
            }
            BASTORE => {
                self.pop_n(3);
            }
            INT2BYTE | INT2CHAR | INT2SHORT => {}
            FMUL | FADD | FSUB | FDIV | FMOD => {
                self.pop_n(1);
            }
            CASTORE | IASTORE | FASTORE | SASTORE => {
                self.pop_n(3);
            }
            LASTORE | DASTORE => {
                self.pop_n(4);
            }
            DUP2 => {
                if self.state.stack[(self.state.stacksize - 1) as usize].is_some() {
                    let value1 = self.pop1(ctx);
                    let value2 = self.pop1(ctx);
                    self.push(ctx, value2);
                    self.push(ctx, value1);
                    self.push(ctx, value2);
                    self.push(ctx, value1);
                } else {
                    let value = self.pop2(ctx);
                    self.push(ctx, value);
                    self.push(ctx, value);
                }
            }
            DUP2_X1 => {
                if self.state.stack[(self.state.stacksize - 1) as usize].is_some() {
                    let value1 = self.pop1(ctx);
                    let value2 = self.pop1(ctx);
                    let value3 = self.pop1(ctx);
                    self.push(ctx, value2);
                    self.push(ctx, value1);
                    self.push(ctx, value3);
                    self.push(ctx, value2);
                    self.push(ctx, value1);
                } else {
                    let value1 = self.pop2(ctx);
                    let value2 = self.pop1(ctx);
                    self.push(ctx, value1);
                    self.push(ctx, value2);
                    self.push(ctx, value1);
                }
            }
            DUP2_X2 => {
                if self.state.stack[(self.state.stacksize - 1) as usize].is_some() {
                    let value1 = self.pop1(ctx);
                    let value2 = self.pop1(ctx);
                    if self.state.stack[(self.state.stacksize - 1) as usize].is_some() {
                        // form 1
                        let value3 = self.pop1(ctx);
                        let value4 = self.pop1(ctx);
                        self.push(ctx, value2);
                        self.push(ctx, value1);
                        self.push(ctx, value4);
                        self.push(ctx, value3);
                        self.push(ctx, value2);
                        self.push(ctx, value1);
                    } else {
                        // form 3
                        let value3 = self.pop2(ctx);
                        self.push(ctx, value2);
                        self.push(ctx, value1);
                        self.push(ctx, value3);
                        self.push(ctx, value2);
                        self.push(ctx, value1);
                    }
                } else {
                    let value1 = self.pop2(ctx);
                    if self.state.stack[(self.state.stacksize - 1) as usize].is_some() {
                        // form 2
                        let value2 = self.pop1(ctx);
                        let value3 = self.pop1(ctx);
                        self.push(ctx, value1);
                        self.push(ctx, value3);
                        self.push(ctx, value2);
                        self.push(ctx, value1);
                    } else {
                        // form 4
                        let value2 = self.pop2(ctx);
                        self.push(ctx, value1);
                        self.push(ctx, value2);
                        self.push(ctx, value1);
                    }
                }
            }
            DUP_X2 => {
                let value1 = self.pop1(ctx);
                if self.state.stack[(self.state.stacksize - 1) as usize].is_some() {
                    // form 1
                    let value2 = self.pop1(ctx);
                    let value3 = self.pop1(ctx);
                    self.push(ctx, value1);
                    self.push(ctx, value3);
                    self.push(ctx, value2);
                    self.push(ctx, value1);
                } else {
                    // form 2
                    let value2 = self.pop2(ctx);
                    self.push(ctx, value1);
                    self.push(ctx, value2);
                    self.push(ctx, value1);
                }
            }
            FCMPL | FCMPG => {
                self.pop_n(2);
                let int = self.syms.int_type;
                self.push(ctx, int);
            }
            DCMPL | DCMPG => {
                self.pop_n(4);
                let int = self.syms.int_type;
                self.push(ctx, int);
            }
            SWAP => {
                let value1 = self.pop1(ctx);
                let value2 = self.pop1(ctx);
                self.push(ctx, value1);
                self.push(ctx, value2);
            }
            DADD | DSUB | DMUL | DDIV | DMOD => {
                self.pop_n(2);
            }
            RET => {
                self.mark_dead();
            }
            WIDE => {
                return;
            }
            MONITORENTER | MONITOREXIT => {
                self.pop_n(1);
            }
            _ => panic!("AssertionError: {}", Code::mnem(op)),
        }
        self.postop();
    }
}
impl Code {
    /// Emit an opcode with a one-byte operand field.
    pub fn emitop1(&mut self, ctx: &mut CompilerContext, op: i32, od: i32) {
        self.emitop1_data(ctx, op, od, None);
    }

    /// Emit an opcode with a one-byte operand field and pool constant data.
    fn emitop1_data(
        &mut self,
        ctx: &mut CompilerContext,
        op: i32,
        od: i32,
        data: Option<LoadableConstantRef>,
    ) {
        self.emitop(ctx, op);
        if !self.alive {
            return;
        }
        self.emit1(od);
        match op {
            BIPUSH => {
                let int = self.syms.int_type;
                self.push(ctx, int);
            }
            LDC1 => {
                let constant = data.expect("AssertionError: ldc constant");
                let t = self.types.constant_type(ctx, &*self.syms, &constant);
                self.push(ctx, t);
            }
            _ => panic!("AssertionError: {}", Code::mnem(op)),
        }
        self.postop();
    }

    /// Emit an opcode with a one-byte operand field, widening when needed.
    pub fn emitop1w(&mut self, ctx: &mut CompilerContext, op: i32, od: i32) {
        if od > 0xFF {
            self.emitop(ctx, WIDE);
            self.emitop(ctx, op);
            self.emit2(od);
        } else {
            self.emitop(ctx, op);
            self.emit1(od);
        }
        if !self.alive {
            return;
        }
        match op {
            ILOAD => {
                let int = self.syms.int_type;
                self.push(ctx, int);
            }
            LLOAD => {
                let long = self.syms.long_type;
                self.push(ctx, long);
            }
            FLOAD => {
                let float = self.syms.float_type;
                self.push(ctx, float);
            }
            DLOAD => {
                let double = self.syms.double_type;
                self.push(ctx, double);
            }
            ALOAD => {
                let t = ctx
                    .sym(self.lvar[od as usize].as_ref().expect("local var").sym)
                    .ty
                    .expect("local type");
                self.push(ctx, t);
            }
            LSTORE | DSTORE => {
                self.pop_n(2);
            }
            ISTORE | FSTORE | ASTORE => {
                self.pop_n(1);
            }
            RET => {
                self.mark_dead();
            }
            _ => panic!("AssertionError: {}", Code::mnem(op)),
        }
        self.postop();
    }

    /// Emit an iinc instruction, widening when needed.
    pub fn emitop1w_char(&mut self, ctx: &mut CompilerContext, op: i32, od1: i32, od2: i32) {
        if od1 > 0xFF || !(-128..=127).contains(&od2) {
            self.emitop(ctx, WIDE);
            self.emitop(ctx, op);
            self.emit2(od1);
            self.emit2(od2);
        } else {
            self.emitop(ctx, op);
            self.emit1(od1);
            self.emit1(od2);
        }
        if !self.alive {
            return;
        }
        match op {
            IINC => {}
            _ => panic!("AssertionError: {}", Code::mnem(op)),
        }
    }

    /// Emit an opcode with a two-byte operand field and symbol data.
    pub fn emitop2_symbol(&mut self, ctx: &mut CompilerContext, op: i32, od: i32, data: SymbolId) {
        self.emitop(ctx, op);
        if !self.alive {
            return;
        }
        self.emit2(od);
        match op {
            GETSTATIC => {
                let syms_rc = Rc::clone(&self.syms);
                let syms = &*syms_rc;
                let ty = ctx.sym(data).ty.expect("symbol type");
                let erased = self.types.erasure(ctx, &syms, ty);
                self.push(ctx, erased);
            }
            PUTSTATIC => {
                let syms_rc = Rc::clone(&self.syms);
                let syms = &*syms_rc;
                let ty = ctx.sym(data).ty.expect("symbol type");
                let erased = self.types.erasure(ctx, &syms, ty);
                self.pop_type(ctx, erased);
            }
            GOTO => {
                self.mark_dead();
            }
            PUTFIELD => {
                let syms_rc = Rc::clone(&self.syms);
                let syms = &*syms_rc;
                let ty = ctx.sym(data).ty.expect("symbol type");
                let erased = self.types.erasure(ctx, &syms, ty);
                self.pop_type(ctx, erased);
                self.pop_n(1); // object ref
            }
            GETFIELD => {
                self.pop_n(1); // object ref
                let syms_rc = Rc::clone(&self.syms);
                let syms = &*syms_rc;
                let ty = ctx.sym(data).ty.expect("symbol type");
                let erased = self.types.erasure(ctx, &syms, ty);
                self.push(ctx, erased);
            }
            JSR => {}
            _ => panic!("AssertionError: {}", Code::mnem(op)),
        }
        self.postop();
    }

    /// Emit an opcode with a two-byte operand field and a type operand.
    pub fn emitop2_type(&mut self, ctx: &mut CompilerContext, op: i32, od: i32, data: TypeId) {
        self.emitop(ctx, op);
        if !self.alive {
            return;
        }
        self.emit2(od);
        match op {
            NEW => {
                let syms_rc = Rc::clone(&self.syms);
                let syms = &*syms_rc;
                let tsym = ctx.typ(data).tsym.expect("class type symbol");
                let owner_ty = ctx.sym(tsym).ty.expect("class symbol type");
                let erased = self.types.erasure(ctx, &syms, owner_ty);
                let uninit =
                    crate::jvm::uninitialized_type::uninitialized_object(ctx, erased, self.cp - 3);
                self.push(ctx, uninit);
            }
            CHECKCAST => {
                self.pop_n(1); // object ref
                let syms_rc = Rc::clone(&self.syms);
                let syms = &*syms_rc;
                let erased = self.types.erasure(ctx, &syms, data);
                self.push(ctx, erased);
            }
            INSTANCEOF => {
                // Java: Code.java:1068-1071 (instanceof: pop 1, push int).
                self.pop_n(1);
                let int = self.syms.int_type;
                self.push(ctx, int);
            }
            _ => panic!("AssertionError: {}", Code::mnem(op)),
        }
        self.postop();
    }

    /// Emit an opcode with a two-byte operand field.
    pub fn emitop2(&mut self, ctx: &mut CompilerContext, op: i32, od: i32) {
        self.emitop2_data(ctx, op, od, None);
    }

    /// Emit an opcode with a two-byte operand field and optional data.
    pub fn emitop2_data(
        &mut self,
        ctx: &mut CompilerContext,
        op: i32,
        od: i32,
        data: Option<LoadableConstantRef>,
    ) {
        self.emitop(ctx, op);
        if !self.alive {
            return;
        }
        self.emit2(od);
        match op {
            SIPUSH => {
                let int = self.syms.int_type;
                self.push(ctx, int);
            }
            IF_ACMP_NULL | IF_ACMP_NONNULL | IFEQ | IFNE | IFLT | IFGE | IFGT | IFLE => {
                self.pop_n(1);
            }
            IF_ICMPEQ | IF_ICMPNE | IF_ICMPLT | IF_ICMPGE | IF_ICMPGT | IF_ICMPLE | IF_ACMPEQ
            | IF_ACMPNE => {
                self.pop_n(2);
            }
            GOTO => {
                self.mark_dead();
            }
            INSTANCEOF => {
                self.pop_n(1);
                let int = self.syms.int_type;
                self.push(ctx, int);
            }
            LDC2 | LDC2W => {
                let constant = data.expect("AssertionError: ldc constant");
                let t = self.types.constant_type(ctx, &*self.syms, &constant);
                self.push(ctx, t);
            }
            JSR => {}
            _ => panic!("AssertionError: {}", Code::mnem(op)),
        }
        self.postop();
    }

    /// Emit an opcode with a four-byte operand field.
    pub fn emitop4(&mut self, ctx: &mut CompilerContext, op: i32, od: i32) {
        self.emitop(ctx, op);
        if !self.alive {
            return;
        }
        self.emit4(od);
        match op {
            GOTO_W => {
                self.mark_dead();
            }
            JSR_W => {}
            _ => panic!("AssertionError: {}", Code::mnem(op)),
        }
    }

    /// Align the code pointer to the next incr boundary.
    pub fn align(&mut self, ctx: &mut CompilerContext, incr: i32) {
        if self.alive {
            while self.cp % incr != 0 {
                self.emitop0(ctx, NOP);
            }
        }
    }

    /// Place a byte into code at address pc.
    fn put1(&mut self, pc: i32, op: i32) {
        self.code[pc as usize] = (op & 0xFF) as u8;
    }

    /// Place two bytes into code at address pc.
    fn put2(&mut self, pc: i32, od: i32) {
        self.put1(pc, od >> 8);
        self.put1(pc + 1, od);
    }

    /// Place four bytes into code at address pc.
    pub fn put4(&mut self, pc: i32, od: i32) {
        self.put1(pc, od >> 24);
        self.put1(pc + 1, od >> 16);
        self.put1(pc + 2, od >> 8);
        self.put1(pc + 3, od);
    }

    /// Return the code byte at position pc as an unsigned int.
    fn get1(&self, pc: i32) -> i32 {
        self.code[pc as usize] as i32 & 0xFF
    }

    /// Return the two code bytes at position pc as an unsigned int.
    fn get2(&self, pc: i32) -> i32 {
        (self.get1(pc) << 8) | self.get1(pc + 1)
    }

    /// Return the four code bytes at position pc as an int.
    pub fn get4(&self, pc: i32) -> i32 {
        (self.get1(pc) << 24)
            | (self.get1(pc + 1) << 16)
            | (self.get1(pc + 2) << 8)
            | self.get1(pc + 3)
    }

    /// Return whether code generation is currently enabled.
    pub fn is_alive(&self) -> bool {
        self.alive || self.pending_jumps.is_some()
    }

    /// Switch code generation off.
    pub fn mark_dead(&mut self) {
        self.alive = false;
    }

    /// Declare an entry point and return the current code pointer.
    pub fn entry_point(&mut self, ctx: &mut CompilerContext) -> i32 {
        let pc = self.cur_cp(ctx);
        self.alive = true;
        self.pending_stack_map = self.need_stack_map;
        pc
    }

    /// Declare an entry point with an initial state and return the code pointer.
    pub fn entry_point_state(&mut self, ctx: &mut CompilerContext, state: &State) -> i32 {
        let pc = self.cur_cp(ctx);
        self.alive = true;
        let new_state = state.dup();
        self.set_defined(ctx, &new_state.defined);
        self.state = new_state;
        assert!(
            state.stacksize <= self.max_stack,
            "AssertionError: entry point stack too large"
        );
        if self.debug_code {
            eprintln!("entry point {}", state.dump());
        }
        self.pending_stack_map = self.need_stack_map;
        pc
    }

    /// Declare an entry point with an initial state plus a pushed value.
    pub fn entry_point_state_pushed(
        &mut self,
        ctx: &mut CompilerContext,
        state: &State,
        pushed: TypeId,
    ) -> i32 {
        let pc = self.cur_cp(ctx);
        self.alive = true;
        let new_state = state.dup();
        self.set_defined(ctx, &new_state.defined);
        self.state = new_state;
        assert!(
            state.stacksize <= self.max_stack,
            "AssertionError: entry point stack too large"
        );
        self.push(ctx, pushed);
        if self.debug_code {
            eprintln!("entry point {}", state.dump());
        }
        self.pending_stack_map = self.need_stack_map;
        pc
    }

    /// Swap the let-expression stack position and return the old one.
    pub fn set_let_expr_stack_pos(&mut self, pos: i32) -> i32 {
        let res = self.let_expr_stack_pos;
        self.let_expr_stack_pos = pos;
        res
    }

    /// Return whether the current position is the start of a statement.
    pub fn is_statement_start(&self) -> bool {
        !self.alive || self.state.stacksize == self.let_expr_stack_pos
    }

    /// Check the code against the JVM class file limits.
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - pos: the diagnostic position of the code;
    ///   - log: the diagnostic log.
    ///
    /// # Returns
    ///   - true when a limit is exceeded (and reported).
    ///
    /// # Panics
    ///   - none.
    pub fn check_limits(&self, pos: i32, log: &Rc<Log>) -> bool {
        if self.cp > class_file::MAX_CODE {
            crate::util::abstract_log::AbstractLog::error_at(
                &*log,
                pos,
                Error::from(errors::limit_code()),
            );
            return true;
        }
        if self.max_locals > class_file::MAX_LOCALS {
            crate::util::abstract_log::AbstractLog::error_at(
                &*log,
                pos,
                Error::from(errors::limit_locals()),
            );
            return true;
        }
        if self.max_stack > class_file::MAX_STACK {
            crate::util::abstract_log::AbstractLog::error_at(
                &*log,
                pos,
                Error::from(errors::limit_stack()),
            );
            return true;
        }
        false
    }

    /// Add a catch clause to the exception table.
    pub fn add_catch(&mut self, start_pc: u16, end_pc: u16, handler_pc: u16, catch_type: u16) {
        self.catch_info
            .push([start_pc, end_pc, handler_pc, catch_type]);
    }

    /// Compress the exception table by dropping empty ranges.
    pub fn compress_catch_table(&mut self) {
        let mut handler_pcs: Vec<i32> = Vec::new();
        for entry in &self.catch_info {
            handler_pcs.push(entry[2] as i32);
        }
        let mut compressed: Vec<[u16; 4]> = Vec::new();
        for entry in &self.catch_info {
            let startpc = entry[0] as i32;
            let endpc = entry[1] as i32;
            if startpc == endpc || (startpc == endpc - 1 && handler_pcs.contains(&startpc)) {
                continue;
            }
            compressed.push(*entry);
        }
        self.catch_info = compressed;
    }

    /// Add a line number entry.
    pub fn add_line_number(&mut self, start_pc: u16, line_number: u16) {
        if self.line_debug_info {
            if self.line_info.non_empty() && self.line_info.head()[0] == start_pc {
                self.line_info = self.line_info.tail();
            }
            if self.line_info.is_empty() || self.line_info.head()[1] != line_number {
                self.line_info = self.line_info.prepend([start_pc, line_number]);
            }
        }
    }

    /// Mark the beginning of a statement.
    pub fn stat_begin(&mut self, pos: i32) {
        if pos != NOPOS {
            self.pending_stat_pos = pos;
        }
    }

    /// Force the statement beginning eagerly.
    pub fn mark_stat_begin(&mut self, ctx: &mut CompilerContext) {
        if self.alive && self.line_debug_info {
            let line_map = self.line_map.as_ref().expect("line map").clone();
            let line = line_map.get_line_number(self.pending_stat_pos);
            let cp1 = (self.cp & 0xFFFF) as u16;
            let line1 = (line & 0xFFFF) as u16;
            if cp1 as i32 == self.cp && line1 as i32 == line {
                self.add_line_number(cp1, line1);
            }
        }
        self.pending_stat_pos = NOPOS;
        let _ = ctx;
    }
}

impl Code {
    /// Emit a stack map at the current PC.
    pub fn emit_stack_map(&mut self, ctx: &mut CompilerContext) {
        let pc = self.cur_cp(ctx);
        if !self.need_stack_map {
            return;
        }
        let locals_size = self.get_locals_size(ctx);
        match self.stack_map {
            StackMapFormat::CLDC => self.emit_cldc_stack_map(ctx, pc, locals_size),
            StackMapFormat::JSR202 => self.emit_stack_map_frame(ctx, pc, locals_size),
            StackMapFormat::NONE => {
                panic!("AssertionError: no stack map format chosen")
            }
        }
        if self.debug_code {
            self.state.dump();
        }
    }

    /// Return the number of local variable slots that are live.
    fn get_locals_size(&mut self, ctx: &mut CompilerContext) -> i32 {
        let mut next_local = 0;
        let mut i = self.max_locals - 1;
        while i >= 0 {
            if self.state.defined.is_member(i)
                && let Some(lv) = &self.lvar[i as usize]
            {
                let syms_rc = Rc::clone(&self.syms);
                let syms = &*syms_rc;
                let ty = ctx.sym(lv.sym).ty.expect("local type");
                let erased = self.types.erasure(ctx, &syms, ty);
                next_local = i + Code::width(Code::typecode(ctx, erased));
                break;
            }
            i -= 1;
        }
        next_local
    }

    /// Emit a CLDC stack map frame.
    fn emit_cldc_stack_map(&mut self, ctx: &mut CompilerContext, pc: i32, locals_size: i32) {
        if self.last_stack_map_pc == pc {
            self.stack_map_buffer_size -= 1;
            self.stack_map_buffer[self.stack_map_buffer_size as usize] = StackMapFrame::new();
        }
        self.last_stack_map_pc = pc;
        if self.stack_map_buffer.len() <= self.stack_map_buffer_size as usize {
            self.stack_map_buffer.resize(
                (self.stack_map_buffer_size + 20) as usize,
                StackMapFrame::new(),
            );
        }
        let mut frame = StackMapFrame::new();
        frame.pc = pc;
        frame.locals = vec![None; locals_size as usize];
        for i in 0..locals_size {
            if self.state.defined.is_member(i)
                && let Some(lv) = &self.lvar[i as usize]
            {
                let syms_rc = Rc::clone(&self.syms);
                let syms = &*syms_rc;
                let ty = ctx.sym(lv.sym).ty.expect("local type");
                let vtype = if ctx.typ(ty).get_tag() == TypeTag::UNINITIALIZED_THIS
                    || ctx.typ(ty).get_tag() == TypeTag::UNINITIALIZED_OBJECT
                {
                    ty
                } else {
                    self.types.erasure(ctx, &syms, ty)
                };
                frame.locals[i as usize] = Some(vtype);
            }
        }
        frame.stack = Vec::new();
        for i in 0..self.state.stacksize {
            frame
                .stack
                .push(self.state.stack[i as usize].expect("stack type"));
        }
        self.stack_map_buffer[self.stack_map_buffer_size as usize] = frame;
        self.stack_map_buffer_size += 1;
    }

    /// Emit a JSR202 stack map frame.
    fn emit_stack_map_frame(&mut self, ctx: &mut CompilerContext, pc: i32, locals_size: i32) {
        if self.last_frame.is_none() {
            self.last_frame = Some(self.get_initial_frame(ctx));
        } else if self.last_frame.as_ref().expect("last frame").pc == pc {
            // drop the existing stack map at this offset
            self.stack_map_buffer_size -= 1;
            self.stack_map_table_buffer[self.stack_map_buffer_size as usize] =
                StackMapTableFrame::Same { offset_delta: 0 };
            self.last_frame = self.frame_before_last.take();
        }
        let mut frame = StackMapFrame::new();
        frame.pc = pc;
        let mut local_count = 0;
        let mut locals: Vec<Option<TypeId>> = vec![None; locals_size as usize];
        let mut i = 0;
        while i < locals_size {
            if self.state.defined.is_member(i)
                && let Some(lv) = &self.lvar[i as usize]
            {
                let syms_rc = Rc::clone(&self.syms);
                let syms = &*syms_rc;
                let ty = ctx.sym(lv.sym).ty.expect("local type");
                let vtype = if ctx.typ(ty).get_tag() == TypeTag::UNINITIALIZED_THIS
                    || ctx.typ(ty).get_tag() == TypeTag::UNINITIALIZED_OBJECT
                {
                    ty
                } else {
                    self.types.erasure(ctx, &syms, ty)
                };
                locals[i as usize] = Some(vtype);
            }
            local_count += 1;
            if let Some(Some(vtype)) = locals.get(i as usize) {
                let w = Code::width_type(ctx, &*self.syms, Some(*vtype));
                if w > 1 {
                    i += 1;
                }
            }
            i += 1;
        }
        // Java: Code.java:1335-1340 — the frame.locals slot may be null for an
        // undefined local (Code.width(null) == 1, Code.java:284-286).
        frame.locals = Vec::new();
        let mut i = 0;
        let mut j = 0;
        while i < locals_size {
            assert!(j < local_count, "AssertionError: local variable count");
            let local = locals[i as usize];
            frame.locals.push(local);
            let w = match local {
                Some(local) => Code::width_type(ctx, &*self.syms, Some(local)),
                None => 1,
            };
            if w > 1 {
                i += 1;
            }
            i += 1;
            j += 1;
        }
        let mut stack_count = 0;
        for i in 0..self.state.stacksize {
            if self.state.stack[i as usize].is_some() {
                stack_count += 1;
            }
        }
        frame.stack = Vec::new();
        for i in 0..self.state.stacksize {
            if let Some(t) = self.state.stack[i as usize] {
                let syms_rc = Rc::clone(&self.syms);
                let syms = &*syms_rc;
                let erased = self.types.erasure(ctx, &syms, t);
                frame.stack.push(erased);
            }
        }
        let _ = stack_count;
        if self.stack_map_table_buffer.len() <= self.stack_map_buffer_size as usize {
            self.stack_map_table_buffer
                .resize_with((self.stack_map_buffer_size + 20) as usize, || {
                    StackMapTableFrame::Same { offset_delta: 0 }
                });
        }
        let last_frame = self.last_frame.as_ref().expect("last frame");
        let syms_rc = Rc::clone(&self.syms);
        let syms = &*syms_rc;
        let entry = StackMapTableFrame::get_instance(
            ctx,
            &frame,
            last_frame.pc,
            &last_frame.locals,
            &self.types,
            &syms,
        );
        self.stack_map_table_buffer[self.stack_map_buffer_size as usize] = entry;
        self.stack_map_buffer_size += 1;
        self.frame_before_last = self.last_frame.take();
        self.last_frame = Some(frame);
    }

    /// Return the initial stack map frame of the method.
    fn get_initial_frame(&mut self, ctx: &mut CompilerContext) -> StackMapFrame {
        let syms_rc = Rc::clone(&self.syms);
        let syms = &*syms_rc;
        let meth_ty = ctx.sym(self.meth).ty.expect("method type");
        let external = crate::code::symbol::external_type(
            ctx,
            &self.types,
            &syms,
            &Rc::clone(&self.pool_writer.borrow().names),
            self.meth,
        );
        let external_ty = external;
        let arg_types = self.types.get_parameter_types_public(ctx, external_ty);
        let len = {
            let mut l = arg_types.clone();
            let mut count = 0;
            while l.non_empty() {
                count += 1;
                l = l.tail();
            }
            count
        };
        let mut frame = StackMapFrame::new();
        let mut count = 0;
        let is_static = ctx.sym(self.meth).is_static(ctx);
        if !is_static {
            let owner = ctx.sym(self.meth).owner.expect("method owner");
            let this_type = ctx.sym(owner).ty.expect("owner type");
            frame.locals = vec![None; (len + 1) as usize];
            let is_constructor = Code::symbol_is_constructor(ctx, self.meth);
            if is_constructor && this_type != syms.object_type {
                frame.locals[count] = Some(crate::jvm::uninitialized_type::uninitialized_this(
                    ctx, this_type,
                ));
                count += 1;
            } else {
                let erased = self.types.erasure(ctx, &syms, this_type);
                frame.locals[count] = Some(erased);
                count += 1;
            }
        } else {
            frame.locals = vec![None; len as usize];
        }
        let mut args = arg_types.clone();
        while args.non_empty() {
            let arg_type = *args.head();
            let erased = self.types.erasure(ctx, &syms, arg_type);
            frame.locals[count] = Some(erased);
            count += 1;
            args = args.tail();
        }
        let _ = meth_ty;
        frame.pc = -1;
        frame.stack = Vec::new();
        frame
    }
}
impl Code {
    /// Negate a branch opcode.
    pub fn negate(opcode: i32) -> i32 {
        if opcode == IF_ACMP_NULL {
            IF_ACMP_NONNULL
        } else if opcode == IF_ACMP_NONNULL {
            IF_ACMP_NULL
        } else {
            ((opcode + 1) ^ 1) - 1
        }
    }

    /// Emit a jump instruction and return the code pointer to patch.
    pub fn emit_jump(&mut self, ctx: &mut CompilerContext, opcode: i32) -> i32 {
        if self.fatcode {
            if opcode == GOTO || opcode == JSR {
                self.emitop4(ctx, opcode + GOTO_W - GOTO, 0);
            } else {
                self.emitop2(ctx, Code::negate(opcode), 8);
                self.emitop4(ctx, GOTO_W, 0);
                self.alive = true;
                self.pending_stack_map = self.need_stack_map;
            }
            self.cp - 5
        } else {
            self.emitop2(ctx, opcode, 0);
            self.cp - 3
        }
    }

    /// Emit a branch with the given opcode and return its chain.
    pub fn branch(&mut self, ctx: &mut CompilerContext, opcode: i32) -> Option<Box<Chain>> {
        let mut result = None;
        if opcode == GOTO {
            result = self.pending_jumps.take();
        }
        if opcode != JSR && self.is_alive() {
            result = Some(Box::new(Chain::new(
                self.emit_jump(ctx, opcode),
                result,
                self.state.dup(),
            )));
            self.fixed_pc = self.fatcode;
            if opcode == GOTO {
                self.alive = false;
            }
        }
        result
    }

    /// Resolve a chain to point to the given target.
    pub fn resolve_chain(
        &mut self,
        ctx: &mut CompilerContext,
        chain: Option<Box<Chain>>,
        target: i32,
    ) {
        let mut changed = false;
        let mut target = target;
        let mut new_state = self.state.dup();
        let mut current = chain;
        while let Some(node) = current {
            assert!(
                !states_equal(&self.state, &node.state)
                    || target > node.pc
                    || self.is_statement_start(),
                "AssertionError: branch state"
            );
            if target >= self.cp {
                target = self.cp;
            } else if self.get1(target) == GOTO {
                if self.fatcode {
                    target = target + self.get4(target + 1);
                } else {
                    target = target + self.get2(target + 1);
                }
            }
            if self.get1(node.pc) == GOTO
                && node.pc + 3 == target
                && target == self.cp
                && !self.fixed_pc
            {
                // If goto the next instruction, the jump is not needed:
                // compact the code.
                if self.var_debug_info {
                    self.adjust_alive_ranges(self.cp, -3);
                }
                self.cp -= 3;
                target -= 3;
                if node.next.is_none() {
                    // This is the only jump to the target. Exit the loop
                    // without setting new state. The code is reachable
                    // from the instruction before goto_.
                    self.alive = true;
                    break;
                }
            } else {
                if self.fatcode {
                    self.put4(node.pc + 1, target - node.pc);
                } else if target - node.pc < i16::MIN as i32 || target - node.pc > i16::MAX as i32 {
                    self.fatcode = true;
                } else {
                    self.put2(node.pc + 1, target - node.pc);
                }
                assert!(
                    !self.alive
                        || node.state.stacksize == new_state.stacksize
                            && node.state.nlocks == new_state.nlocks,
                    "AssertionError: incompatible branch states"
                );
            }
            self.fixed_pc = true;
            if self.cp == target {
                changed = true;
                if self.debug_code {
                    eprintln!("resolving chain state={}", node.state.dump());
                }
                if self.alive {
                    let syms_rc = Rc::clone(&self.syms);
                    let syms = &*syms_rc;
                    let mut joined = node.state.dup();
                    joined.join(ctx, &syms, &self.types, &new_state);
                    new_state = joined;
                } else {
                    new_state = node.state.clone();
                    self.alive = true;
                }
            }
            current = node.next;
        }
        // Java: Code.java:1530-1535 — upstream compares the State objects by
        // reference (state != newState): newState is a fresh object whenever
        // changed is set, so the assertion always holds and the state is always
        // switched in that case.
        debug_assert!(!changed || changed);
        if changed {
            let defined = new_state.defined.dup();
            self.set_defined(ctx, &defined);
            self.state = new_state;
            self.pending_stack_map = self.need_stack_map;
        }
    }

    /// Resolve a chain to point to the current code pointer.
    pub fn resolve_pending_chain(&mut self, ctx: &mut CompilerContext, chain: Option<Box<Chain>>) {
        assert!(
            !self.alive
                || chain.is_none()
                || self.state.stacksize == chain.as_ref().map(|c| c.state.stacksize).unwrap_or(0)
                    && self.state.nlocks == chain.as_ref().map(|c| c.state.nlocks).unwrap_or(0),
            "AssertionError: incompatible pending chain"
        );
        self.pending_jumps = Code::merge_chains(chain, self.pending_jumps.take());
        let _ = ctx;
    }

    /// Resolve any pending jumps.
    pub fn resolve_pending(&mut self, ctx: &mut CompilerContext) {
        let x = self.pending_jumps.take();
        let target = self.cp;
        self.resolve_chain(ctx, x, target);
    }

    /// Merge two jump chains into one ordered by decreasing pc.
    pub fn merge_chains(
        chain1: Option<Box<Chain>>,
        chain2: Option<Box<Chain>>,
    ) -> Option<Box<Chain>> {
        let Some(c2) = chain2 else {
            return chain1;
        };
        let Some(c1) = chain1 else {
            return Some(c2);
        };
        assert!(
            c1.state.stacksize == c2.state.stacksize && c1.state.nlocks == c2.state.nlocks,
            "AssertionError: incompatible chains"
        );
        if c1.pc < c2.pc {
            let next = Code::merge_chains(Some(c1), c2.next.clone());
            let state = c2.state.clone();
            Some(Box::new(Chain::new(c2.pc, next, state)))
        } else {
            let next = Code::merge_chains(c1.next.clone(), Some(c2));
            let state = c1.state.clone();
            Some(Box::new(Chain::new(c1.pc, next, state)))
        }
    }
}

impl Code {
    /// Add a new local variable.
    fn add_local_var(&mut self, ctx: &mut CompilerContext, v: SymbolId) {
        let adr = Code::var_adr(ctx, v);
        if self.lvar.len() < (adr + 1) as usize {
            self.lvar.resize((adr + 1) as usize, None);
        }
        assert!(
            self.lvar[adr as usize].is_none(),
            "AssertionError: register already defined"
        );
        if self.pending_jumps.is_some() {
            self.resolve_pending(ctx);
        }
        self.lvar[adr as usize] = Some(LocalVar::new(v, adr));
        self.state.defined.excl(adr);
    }

    /// Return the register of a variable symbol.
    fn var_adr(ctx: &CompilerContext, v: SymbolId) -> i32 {
        // Java: VarSymbol.adr — BindingSymbol extends VarSymbol upstream
        // (Symbol.java:1861-1876).
        match ctx.sym(v).local_adr() {
            Some(adr) => adr,
            None => panic!("AssertionError: variable symbol expected"),
        }
    }

    /// Set the register of a variable symbol.
    fn set_var_adr(ctx: &mut CompilerContext, v: SymbolId, adr: i32) {
        // Java: VarSymbol.adr = adr (BindingSymbol inherits it upward;
        // Symbol.java:1861-1876).
        let mut sym = ctx.sym(v).clone();
        if sym.set_local_adr(adr) {
            *ctx.sym_mut(v) = sym;
        }
    }

    /// Adjust all closed live ranges that start at or after oldCP by delta.
    pub fn adjust_alive_ranges(&mut self, old_cp: i32, delta: i32) {
        for lv in self.lvar.iter_mut().flatten() {
            for range in &mut lv.alive_ranges {
                if range.closed() && range.start_pc + range.length >= old_cp {
                    range.length += delta;
                }
            }
        }
    }

    /// Return the size of the LocalVariableTable.
    pub fn get_lvt_size(&self) -> i32 {
        let mut result = self.var_buffer_size;
        for i in 0..self.var_buffer_size {
            let var = self.var_buffer[i as usize]
                .as_ref()
                .expect("AssertionError: local variable table entry");
            result += var.alive_ranges.len() as i32 - 1;
        }
        result
    }

    /// Set the current variable defined state.
    pub fn set_defined(&mut self, ctx: &mut CompilerContext, new_defined: &Bits) {
        if self.alive && !bits_equal(new_defined, &self.state.defined) {
            let mut diff = self.state.defined.dup();
            diff.xor_set(new_defined);
            let mut adr = diff.next_bit(0);
            while adr >= 0 {
                if adr >= self.nextreg {
                    self.state.defined.excl(adr);
                } else if self.state.defined.is_member(adr) {
                    self.set_undefined(ctx, adr);
                } else {
                    self.set_defined_at(ctx, adr);
                }
                adr = diff.next_bit(adr + 1);
            }
        }
    }

    /// Mark a register as being possibly defined.
    pub fn set_defined_at(&mut self, ctx: &mut CompilerContext, adr: i32) {
        let lv = self.lvar[adr as usize].clone();
        match lv {
            None => {
                self.state.defined.excl(adr);
            }
            Some(_v) => {
                self.state.defined.incl(adr);
                if self.cp < u16::MAX as i32 {
                    let cp = self.cp;
                    let ctx_lvar = &mut self.lvar[adr as usize];
                    if let Some(var) = ctx_lvar {
                        var.open_range(cp);
                    }
                }
            }
        }
        let _ = ctx;
    }

    /// Mark a register as being undefined.
    ///
    /// Java: public void setUndefined(int adr) (Code.java:2067-2084): the
    /// closed original `v` is the one put into the var buffer and used to fill
    /// the type annotation positions; the slot keeps a fresh empty dup.
    pub fn set_undefined(&mut self, ctx: &mut CompilerContext, adr: i32) {
        self.state.defined.excl(adr);
        if (adr as usize) < self.lvar.len()
            && self.lvar[adr as usize]
                .as_ref()
                .is_some_and(|var| var.is_last_range_initialized())
        {
            let var = self.lvar[adr as usize].as_ref().expect("local var").clone();
            let start_pc = var.last_range().expect("last range").start_pc;
            let length = self.cur_cp(ctx) - start_pc;
            if length < u16::MAX as i32 {
                self.lvar[adr as usize] = Some(var.dup());
                let mut v = var;
                v.close_range(length);
                self.put_var(ctx, &v);
                self.fill_local_var_position(ctx, &v);
            } else {
                if let Some(var) = &mut self.lvar[adr as usize] {
                    var.remove_last_range();
                }
            }
        }
    }

    /// End the scope of a variable.
    ///
    /// Java: private void endScope(int adr) (Code.java:2086-2106): the closed
    /// range belongs to the variable that is being dropped, so the positions
    /// must be filled before the slot is cleared.
    fn end_scope(&mut self, ctx: &mut CompilerContext, adr: i32) {
        let lv = self.lvar[adr as usize].clone();
        if let Some(var) = lv {
            if var.is_last_range_initialized() {
                let start_pc = var.last_range().expect("last range").start_pc;
                let length = self.cur_cp(ctx) - start_pc;
                if length < u16::MAX as i32 {
                    let mut v = var;
                    v.close_range(length);
                    self.put_var(ctx, &v);
                    self.fill_local_var_position(ctx, &v);
                }
            }
            // the call to curCP() can implicitly adjust the current cp; the
            // assignment of null must therefore be the last action.
            self.lvar[adr as usize] = None;
        }
        self.state.defined.excl(adr);
    }

    /// Fill the local-variable positions of the type annotations on a variable.
    fn fill_local_var_position(&mut self, ctx: &mut CompilerContext, lv: &LocalVar) {
        if !crate::code::symbol::symbol_has_type_annotations(ctx, lv.sym)
            || crate::code::symbol::symbol_is_exception_parameter(ctx, lv.sym)
        {
            return;
        }
        let valid_ranges: Vec<Range> = lv
            .alive_ranges
            .iter()
            .filter(|r| r.closed() && r.length > 0)
            .cloned()
            .collect();
        if valid_ranges.is_empty() {
            return;
        }
        let lvar_offset: Vec<i32> = valid_ranges.iter().map(|r| r.start_pc).collect();
        let lvar_length: Vec<i32> = valid_ranges.iter().map(|r| r.length).collect();
        let lvar_index: Vec<i32> = valid_ranges.iter().map(|_| lv.reg).collect();
        let attrs = crate::code::symbol::symbol_raw_type_attributes(ctx, lv.sym);
        let current = attrs;
        for attr in &current {
            let Some(mut pos) = ctx.attr(attr).get_position(ctx) else {
                continue;
            };
            pos.lvar_offset = Some(append_array(pos.lvar_offset, &lvar_offset));
            pos.lvar_length = Some(append_array(pos.lvar_length, &lvar_length));
            pos.lvar_index = Some(append_array(pos.lvar_index, &lvar_index));
            pos.is_valid_offset = true;
            if let crate::code::attribute::AttributeKind::TypeCompound { position, .. } =
                &mut ctx.attr_mut(attr).kind
            {
                *position = Some(pos);
            }
        }
    }

    /// Fill the exception table indices of exception-parameter type annotations.
    pub fn fill_exception_parameter_positions(&mut self, ctx: &mut CompilerContext) {
        for i in 0..self.var_buffer_size {
            let lv = self.var_buffer[i as usize].clone();
            let Some(lv) = lv else {
                continue;
            };
            if !crate::code::symbol::symbol_has_type_annotations(ctx, lv.sym)
                || !crate::code::symbol::symbol_is_exception_parameter(ctx, lv.sym)
            {
                continue;
            }
            let attrs = crate::code::symbol::symbol_raw_type_attributes(ctx, lv.sym);
            let current = attrs;
            for attr in &current {
                let Some(mut pos) = ctx.attr(attr).get_position(ctx) else {
                    continue;
                };
                if pos.has_catch_type() {
                    let idx = self.find_exception_index(&pos);
                    assert_ne!(idx, -1, "AssertionError: Could not find exception index for type annotation");
                    pos.set_exception_index(idx);
                    if let crate::code::attribute::AttributeKind::TypeCompound {
                        position, ..
                    } = &mut ctx.attr_mut(attr).kind
                    {
                        *position = Some(pos);
                    }
                }
            }
        }
    }

    /// Return the exception table index of an exception-parameter annotation.
    fn find_exception_index(
        &self,
        pos: &crate::code::type_annotation_position::TypeAnnotationPosition,
    ) -> i32 {
        let catch_type = pos.get_catch_type();
        let start_pos = pos.get_start_pos();
        for (i, entry) in self.catch_info.iter().enumerate() {
            if catch_type == entry[3] as i32 && start_pos == entry[0] as i32 {
                return i as i32;
            }
        }
        -1
    }

    /// Put a live variable range into the output buffer.
    fn put_var(&mut self, ctx: &mut CompilerContext, var: &LocalVar) {
        // Keep local variables if
        // 1) we need them for debug information
        // 2) it is an exception type and it contains type annotations
        let keep_local_variables = self.var_debug_info
            || (crate::code::symbol::symbol_is_exception_parameter(ctx, var.sym)
                && crate::code::symbol::symbol_has_type_annotations(ctx, var.sym));
        if !keep_local_variables {
            return;
        }
        // don't keep synthetic vars, unless they are lambda method parameters
        let sym = ctx.sym(var.sym);
        let ignored_synthetic_var = (sym.flags_field & crate::code::flags::SYNTHETIC as i64) != 0
            && (sym
                .owner
                .map(|owner| (ctx.sym(owner).flags_field & crate::code::flags::LAMBDA_METHOD) == 0)
                .unwrap_or(true)
                || (sym.flags_field & crate::code::flags::PARAMETER) == 0);
        if ignored_synthetic_var {
            return;
        }
        if self.var_buffer.len() <= self.var_buffer_size as usize {
            self.var_buffer
                .resize((self.var_buffer_size + 20) as usize, None);
        }
        self.var_buffer[self.var_buffer_size as usize] = Some(var.clone());
        self.var_buffer_size += 1;
    }

    /// Create a new local variable address.
    fn new_local(&mut self, typecode: i32) -> i32 {
        let reg = self.nextreg;
        let w = Code::width(typecode);
        self.nextreg = reg + w;
        if self.nextreg > self.max_locals {
            self.max_locals = self.nextreg;
        }
        // Java: lvar = ArrayUtils.ensureCapacity(lvar, nextreg)
        // (Code.java:newLocal); synthetic locals use newLocal without
        // addLocalVar, so the register array must cover nextreg here.
        if self.lvar.len() < self.nextreg as usize {
            self.lvar.resize(self.nextreg as usize, None);
        }
        reg
    }

    /// Create a new local variable address for a type.
    fn new_local_type(&mut self, ctx: &mut CompilerContext, type_: TypeId) -> i32 {
        let tc = Code::typecode(ctx, type_);
        self.new_local(tc)
    }

    /// Create a new local variable for a symbol and add it.
    pub fn new_local_symbol(&mut self, ctx: &mut CompilerContext, v: SymbolId) -> i32 {
        let syms_rc = Rc::clone(&self.syms);
        let syms = &*syms_rc;
        let ty = ctx.sym(v).ty.expect("variable type");
        let erased = self.types.erasure(ctx, &syms, ty);
        let reg = self.new_local_type(ctx, erased);
        Code::set_var_adr(ctx, v, reg);
        self.add_local_var(ctx, v);
        reg
    }

    /// Start a set of fresh registers.
    pub fn new_reg_segment(&mut self) {
        self.nextreg = self.max_locals;
    }

    /// End scopes of all variables with registers greater or equal to first.
    pub fn end_scopes(&mut self, ctx: &mut CompilerContext, first: i32) {
        let prev_next_reg = self.nextreg;
        self.nextreg = first;
        let mut i = self.nextreg;
        while i < prev_next_reg {
            self.end_scope(ctx, i);
            i += 1;
        }
    }
}

/// Append one i32 array to another, returning the concatenation.
fn append_array(source: Option<Vec<i32>>, append: &[i32]) -> Vec<i32> {
    let mut result = source.unwrap_or_default();
    result.extend_from_slice(append);
    result
}

impl Code {
    /// Return the mnemonic of an opcode.
    pub fn mnem(opcode: i32) -> String {
        MNEUMONICS[opcode as usize].to_string()
    }
}

const MNEUMONICS: [&str; 203] = [
    "nop",
    "aconst_null",
    "iconst_m1",
    "iconst_0",
    "iconst_1",
    "iconst_2",
    "iconst_3",
    "iconst_4",
    "iconst_5",
    "lconst_0",
    "lconst_1",
    "fconst_0",
    "fconst_1",
    "fconst_2",
    "dconst_0",
    "dconst_1",
    "bipush",
    "sipush",
    "ldc1",
    "ldc2",
    "ldc2w",
    "iload",
    "lload",
    "fload",
    "dload",
    "aload",
    "iload_0",
    "lload_0",
    "fload_0",
    "dload_0",
    "aload_0",
    "iload_1",
    "lload_1",
    "fload_1",
    "dload_1",
    "aload_1",
    "iload_2",
    "lload_2",
    "fload_2",
    "dload_2",
    "aload_2",
    "iload_3",
    "lload_3",
    "fload_3",
    "dload_3",
    "aload_3",
    "iaload",
    "laload",
    "faload",
    "daload",
    "aaload",
    "baload",
    "caload",
    "saload",
    "istore",
    "lstore",
    "fstore",
    "dstore",
    "astore",
    "istore_0",
    "lstore_0",
    "fstore_0",
    "dstore_0",
    "astore_0",
    "istore_1",
    "lstore_1",
    "fstore_1",
    "dstore_1",
    "astore_1",
    "istore_2",
    "lstore_2",
    "fstore_2",
    "dstore_2",
    "astore_2",
    "istore_3",
    "lstore_3",
    "fstore_3",
    "dstore_3",
    "astore_3",
    "iastore",
    "lastore",
    "fastore",
    "dastore",
    "aastore",
    "bastore",
    "castore",
    "sastore",
    "pop",
    "pop2",
    "dup",
    "dup_x1",
    "dup_x2",
    "dup2",
    "dup2_x1",
    "dup2_x2",
    "swap",
    "iadd",
    "ladd",
    "fadd",
    "dadd",
    "isub",
    "lsub",
    "fsub",
    "dsub",
    "imul",
    "lmul",
    "fmul",
    "dmul",
    "idiv",
    "ldiv",
    "fdiv",
    "ddiv",
    "imod",
    "lmod",
    "fmod",
    "dmod",
    "ineg",
    "lneg",
    "fneg",
    "dneg",
    "ishl",
    "lshl",
    "ishr",
    "lshr",
    "iushr",
    "lushr",
    "iand",
    "land",
    "ior",
    "lor",
    "ixor",
    "lxor",
    "iinc",
    "i2l",
    "i2f",
    "i2d",
    "l2i",
    "l2f",
    "l2d",
    "f2i",
    "f2l",
    "f2d",
    "d2i",
    "d2l",
    "d2f",
    "int2byte",
    "int2char",
    "int2short",
    "lcmp",
    "fcmpl",
    "fcmpg",
    "dcmpl",
    "dcmpg",
    "ifeq",
    "ifne",
    "iflt",
    "ifge",
    "ifgt",
    "ifle",
    "if_icmpeq",
    "if_icmpne",
    "if_icmplt",
    "if_icmpge",
    "if_icmpgt",
    "if_icmple",
    "if_acmpeq",
    "if_acmpne",
    "goto_",
    "jsr",
    "ret",
    "tableswitch",
    "lookupswitch",
    "ireturn",
    "lreturn",
    "freturn",
    "dreturn",
    "areturn",
    "return_",
    "getstatic",
    "putstatic",
    "getfield",
    "putfield",
    "invokevirtual",
    "invokespecial",
    "invokestatic",
    "invokeinterface",
    "invokedynamic",
    "new_",
    "newarray",
    "anewarray",
    "arraylength",
    "athrow",
    "checkcast",
    "instanceof_",
    "monitorenter",
    "monitorexit",
    "wide",
    "multianewarray",
    "if_acmp_null",
    "if_acmp_nonnull",
    "goto_w",
    "jsr_w",
    "breakpoint",
];

/// Compare two bit sets by their members.
fn bits_equal(a: &Bits, b: &Bits) -> bool {
    let mut i = a.next_bit(0);
    while i >= 0 {
        if !b.is_member(i) {
            return false;
        }
        i = a.next_bit(i + 1);
    }
    let mut j = b.next_bit(0);
    while j >= 0 {
        if !a.is_member(j) {
            return false;
        }
        j = b.next_bit(j + 1);
    }
    true
}

/// Compare two machine states structurally.
fn states_equal(a: &State, b: &State) -> bool {
    bits_equal(&a.defined, &b.defined)
        && a.stacksize == b.stacksize
        && a.nlocks == b.nlocks
        && a.stack == b.stack
}

impl Code {
    /// Return the type of a jsr return value.
    ///
    /// The upstream field is a fresh INT primitive type created at class
    /// initialization; the ported methods do not reference it, so the port
    /// returns the shared primitive int type of the symbol table.
    ///
    /// # Arguments
    ///   - syms: the symbol table.
    ///
    /// # Returns
    ///   - the int primitive type.
    ///
    /// # Panics
    ///   - none.
    pub fn jsr_return_value(syms: &Symtab) -> TypeId {
        syms.int_type
    }
}
