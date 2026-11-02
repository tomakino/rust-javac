//! Code-generation items: addressable entities in the bytecode.

use crate::ids::{SymbolId, TreeId, TypeId};
use crate::jvm::code::Chain;
use crate::jvm::pool_constant::LoadableConstantRef;
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
