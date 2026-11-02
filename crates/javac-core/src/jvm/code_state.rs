//! Code-generation state types: live ranges, local variables, the simulated
//! machine state and jump chains.

use crate::code::symtab::Symtab;
use crate::code::type_tag::TypeTag;
use crate::code::types::Types;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TypeId};
use crate::jvm::code::Code;
use crate::util::bits::Bits;

/// One local variable live range.
#[derive(Clone, Debug)]
pub struct Range {
    /// The first code position of the range.
    pub start_pc: i32,
    /// The length of the range.
    pub length: i32,
}

impl Range {
    /// Create an uninitialized range.
    pub fn new() -> Range {
        Range {
            start_pc: u16::MAX as i32,
            length: u16::MAX as i32,
        }
    }

    /// Create a range starting at the given position.
    pub fn with_start(start: i32) -> Range {
        Range {
            start_pc: start,
            length: u16::MAX as i32,
        }
    }

    /// Create a closed range.
    pub fn with(start: i32, length: i32) -> Range {
        Range {
            start_pc: start,
            length,
        }
    }

    /// Return whether both bounds are initialized.
    pub fn closed(&self) -> bool {
        self.start_pc != u16::MAX as i32 && self.length != u16::MAX as i32
    }
}

impl Default for Range {
    fn default() -> Range {
        Range::new()
    }
}

/// A local variable with its live ranges.
#[derive(Clone, Debug)]
pub struct LocalVar {
    /// The variable symbol.
    pub sym: SymbolId,
    /// The register of the variable.
    pub reg: i32,
    /// The live ranges of the variable.
    pub alive_ranges: Vec<Range>,
}

impl LocalVar {
    /// Create a local variable for a symbol.
    pub fn new(sym: SymbolId, reg: i32) -> LocalVar {
        LocalVar {
            sym,
            reg,
            alive_ranges: Vec::new(),
        }
    }

    /// Return a copy of this variable with the same symbol and register.
    pub fn dup(&self) -> LocalVar {
        LocalVar::new(self.sym, self.reg)
    }

    /// Return the first live range, if any.
    pub fn first_range(&self) -> Option<&Range> {
        self.alive_ranges.first()
    }

    /// Return the last live range, if any.
    pub fn last_range(&self) -> Option<&Range> {
        self.alive_ranges.last()
    }

    /// Remove the last live range.
    pub fn remove_last_range(&mut self) {
        self.alive_ranges.pop();
    }

    /// Open a new range starting at the given position when none is open.
    pub fn open_range(&mut self, start: i32) {
        if !self.has_open_range() {
            self.alive_ranges.push(Range::with_start(start));
        }
    }

    /// Close the last range with the given length.
    pub fn close_range(&mut self, length: i32) {
        if self.is_last_range_initialized() && length > 0 {
            if let Some(range) = self.alive_ranges.last_mut()
                && range.length == u16::MAX as i32
            {
                range.length = length;
            }
        } else {
            self.remove_last_range();
        }
    }

    /// Return whether the last range is still open.
    pub fn has_open_range(&self) -> bool {
        match self.alive_ranges.last() {
            Some(range) => range.length == u16::MAX as i32,
            None => false,
        }
    }

    /// Return whether the last range has an initialized start.
    pub fn is_last_range_initialized(&self) -> bool {
        match self.alive_ranges.last() {
            Some(range) => range.start_pc != u16::MAX as i32,
            None => false,
        }
    }

    /// Return the widest range covering all live ranges.
    pub fn get_widest_range(&self) -> Range {
        match (self.first_range(), self.last_range()) {
            (Some(first), Some(last)) => {
                let length = last.length + (last.start_pc - first.start_pc);
                Range::with(first.start_pc, length)
            }
            _ => Range::new(),
        }
    }
}

/// The simulated VM machine state (registers and operand stack).
#[derive(Debug)]
pub struct State {
    /// The set of registers containing values.
    pub defined: Bits,
    /// The types of the operand stack entries; None marks the upper half of a
    /// wide value.
    pub stack: Vec<Option<TypeId>>,
    /// The first unused stack position.
    pub stacksize: i32,
    /// The registers containing locked monitors.
    pub locks: Vec<i32>,
    /// The number of locked monitors.
    pub nlocks: i32,
}

impl Default for State {
    /// Create an empty machine state.
    fn default() -> State {
        State::new()
    }
}

impl Clone for State {
    /// Return a copy of this state, as used by jump chain clones.
    fn clone(&self) -> State {
        self.dup()
    }
}

impl State {
    /// Create an empty machine state.
    pub fn new() -> State {
        State {
            defined: Bits::new(),
            stack: Vec::new(),
            stacksize: 0,
            locks: Vec::new(),
            nlocks: 0,
        }
    }

    /// Return a deep copy of this state.
    pub fn dup(&self) -> State {
        let mut defined = Bits::new();
        let mut bit = self.defined.next_bit(0);
        while bit >= 0 {
            defined.incl(bit);
            bit = self.defined.next_bit(bit + 1);
        }
        State {
            defined,
            stack: self.stack.clone(),
            stacksize: self.stacksize,
            locks: self.locks.clone(),
            nlocks: self.nlocks,
        }
    }

    /// Lock a register (enter a monitor).
    pub fn lock(&mut self, register: i32) {
        // Java: locks[nlocks] = register; nlocks++ (Code.java:1687-1695).
        // The slot is reused after unlock, not appended: push would make a
        // later unlock inspect a stale -1 instead of the new monitor.
        let index = self.nlocks as usize;
        if self.locks.len() <= index {
            self.locks.resize(index + 1, -1);
        }
        self.locks[index] = register;
        self.nlocks += 1;
    }

    /// Unlock a register (exit a monitor).
    pub fn unlock(&mut self, register: i32) {
        self.nlocks -= 1;
        let expected = self.locks[self.nlocks as usize];
        assert!(expected == register, "AssertionError: monitor stack mismatch");
        self.locks[self.nlocks as usize] = -1;
    }

    /// Push a value of the given type onto the operand stack.
    ///
    /// Byte/short/char/boolean values are widened to int; wide values occupy two
    /// stack slots. The caller is responsible for updating Code.max_stack.
    pub fn push(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: TypeId,
    ) {
        let mut t = t;
        match ctx.typ(t).get_tag() {
            TypeTag::VOID => return,
            TypeTag::BYTE | TypeTag::CHAR | TypeTag::SHORT | TypeTag::BOOLEAN => {
                t = syms.int_type;
            }
            _ => {}
        }
        let width = Code::width_type(ctx, syms, Some(t));
        if self.stack.len() < (self.stacksize + 2) as usize {
            self.stack.resize((self.stacksize + 2) as usize, None);
        }
        self.stack[self.stacksize as usize] = Some(t);
        self.stacksize += 1;
        match width {
            1 => {}
            2 => {
                self.stack[self.stacksize as usize] = None;
                self.stacksize += 1;
            }
            _ => panic!("AssertionError: bad verifier width"),
        }
    }

    /// Pop one single-width value.
    pub fn pop1(&mut self, ctx: &mut CompilerContext, syms: &Symtab) -> TypeId {
        self.stacksize -= 1;
        let result = self.stack[self.stacksize as usize]
            .expect("AssertionError: empty operand stack");
        self.stack[self.stacksize as usize] = None;
        assert!(
            Code::width_type(ctx, syms, Some(result)) == 1,
            "AssertionError: expected single-width value"
        );
        result
    }

    /// Return the topmost stack value.
    pub fn peek(&self) -> TypeId {
        self.stack[(self.stacksize - 1) as usize].expect("AssertionError: empty operand stack")
    }

    /// Pop one two-slot value.
    pub fn pop2(&mut self, ctx: &mut CompilerContext, syms: &Symtab) -> TypeId {
        self.stacksize -= 2;
        let result = self.stack[self.stacksize as usize]
            .expect("AssertionError: empty operand stack");
        self.stack[self.stacksize as usize] = None;
        assert!(
            self.stack[(self.stacksize + 1) as usize].is_none(),
            "AssertionError: expected a wide value"
        );
        assert!(
            Code::width_type(ctx, syms, Some(result)) == 2,
            "AssertionError: expected a wide value"
        );
        result
    }

    /// Pop n slots.
    pub fn pop(&mut self, n: i32) {
        let mut n = n;
        while n > 0 {
            self.stacksize -= 1;
            self.stack[self.stacksize as usize] = None;
            n -= 1;
        }
    }

    /// Pop a value of the given type (its verifier width).
    pub fn pop_type(&mut self, ctx: &mut CompilerContext, syms: &Symtab, t: TypeId) {
        let width = Code::width_type(ctx, syms, Some(t));
        self.pop(width);
    }
}

impl State {
    /// Force the top of the stack to be treated as the given supertype.
    ///
    /// Only reference types are rewritten; the current type must be a subtype of
    /// the forced type.
    pub fn force_stack_top(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        t: TypeId,
    ) {
        match ctx.typ(t).get_tag() {
            TypeTag::CLASS | TypeTag::ARRAY => {
                let width = Code::width_type(ctx, syms, Some(t));
                let old = self.stack[(self.stacksize - width) as usize]
                    .expect("AssertionError: empty operand stack");
                let old_erased = types.erasure(ctx, syms, old);
                let t_erased = types.erasure(ctx, syms, t);
                assert!(
                    types.is_subtype(ctx, syms, old_erased, t_erased),
                    "AssertionError: forced stack top is not a subtype"
                );
                self.stack[(self.stacksize - width) as usize] = Some(t);
            }
            _ => {}
        }
    }

    /// Join this state with another state at a control-flow merge point.
    pub fn join(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        types: &Types,
        other: &State,
    ) {
        self.defined.and_set(&other.defined);
        assert!(
            self.stacksize == other.stacksize && self.nlocks == other.nlocks,
            "AssertionError: inconsistent state sizes at join point"
        );
        let mut i = 0;
        while i < self.stacksize {
            let t = self.stack[i as usize];
            let tother = other.stack[i as usize];
            let result = if t == tother {
                t
            } else {
                match (t, tother) {
                    (Some(t), Some(tother))
                        if types.is_subtype(ctx, syms, t, tother) =>
                    {
                        Some(tother)
                    }
                    (Some(t), Some(tother))
                        if types.is_subtype(ctx, syms, tother, t) =>
                    {
                        Some(t)
                    }
                    _ => panic!(
                        "AssertionError: inconsistent stack types at join point"
                    ),
                }
            };
            let result = result.expect("AssertionError: empty stack slot at join point");
            let w = Code::width_type(ctx, syms, Some(result));
            self.stack[i as usize] = Some(result);
            if w == 2 {
                assert!(
                    self.stack[(i + 1) as usize].is_none(),
                    "AssertionError: wide value stack layout"
                );
            }
            i += w;
        }
    }

    /// Return a one-line description of this state (debug output).
    pub fn dump(&self) -> String {
        format!(
            "State{{stacksize={}, nlocks={}}}",
            self.stacksize, self.nlocks
        )
    }
}

/// A chain of unresolved jumps, sorted by decreasing code position.
#[derive(Clone, Debug)]
pub struct Chain {
    /// The position of the jump instruction.
    pub pc: i32,
    /// The machine state after the jump instruction.
    pub state: State,
    /// The next jump in the list.
    pub next: Option<Box<Chain>>,
}

impl Chain {
    /// Create a jump chain node.
    pub fn new(pc: i32, next: Option<Box<Chain>>, state: State) -> Chain {
        Chain { pc, state, next }
    }
}

