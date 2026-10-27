// Source: com/sun/tools/javac/jvm/Code.java (StackMapFormat/StackMapFrame) and
//         com/sun/tools/javac/jvm/ClassWriter.java (StackMapTableFrame hierarchy)

//! Stack map support shared by the code generator and the class writer.
//!
//! The code generator records stack map frames as VerifierType arrays; the class
//! writer compresses consecutive frames into JSR202 StackMapTable entries.

use crate::code::symtab::Symtab;
use crate::code::type_tag::TypeTag;
use crate::code::types::Types;
use crate::compiler_context::CompilerContext;
use crate::ids::TypeId;
use crate::util::name::Name;
use crate::util::names::Names;

/// The format of the stack map attribute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StackMapFormat {
    /// No stack map attribute is generated.
    NONE,
    /// The CLDC StackMap attribute.
    CLDC,
    /// The JSR202 StackMapTable attribute.
    JSR202,
}

impl StackMapFormat {
    /// Return the attribute name of this format.
    pub fn get_attribute_name(self, names: &Names) -> Name {
        match self {
            StackMapFormat::CLDC => names.StackMap.clone(),
            StackMapFormat::JSR202 => names.StackMapTable.clone(),
            StackMapFormat::NONE => names.empty.clone(),
        }
    }
}

/// One stack map frame recorded by the code generator.
///
/// Undefined local variable slots are represented by None (Java null).
#[derive(Clone, Debug)]
pub struct StackMapFrame {
    /// The code position of the frame (-1 for the initial frame).
    pub pc: i32,
    /// The local variable types; None marks an undefined slot.
    pub locals: Vec<Option<TypeId>>,
    /// The operand stack types.
    pub stack: Vec<TypeId>,
}

impl Default for StackMapFrame {
    /// Create an empty frame.
    fn default() -> StackMapFrame {
        StackMapFrame::new()
    }
}

impl StackMapFrame {
    /// Create an empty frame.
    pub fn new() -> StackMapFrame {
        StackMapFrame {
            pc: 0,
            locals: Vec::new(),
            stack: Vec::new(),
        }
    }
}

/// Constant used by the compressed StackMapTable encoding.
pub const SAME_FRAME_SIZE: i32 = 64;
/// Frame type of an extended same-locals-one-stack-item frame.
pub const SAME_LOCALS_1_STACK_ITEM_EXTENDED: i32 = 247;
/// Frame type of the extended same frame.
pub const SAME_FRAME_EXTENDED: i32 = 251;
/// Frame type of a full frame.
pub const FULL_FRAME: i32 = 255;
/// The maximum local variable length difference encoded compactly.
pub const MAX_LOCAL_LENGTH_DIFF: i32 = 4;

/// The writer interface used by a stack map table frame.
///
/// The upstream frames write into the enclosing ClassWriter's data buffer and
/// ask it for the encoding of one verifier type; the port keeps this contract as
/// a trait so the frame hierarchy does not depend on the whole class writer.
pub trait StackMapTableWriter {
    /// Append one byte to the class data buffer.
    fn append_byte(&mut self, b: i32);
    /// Append one unsigned short to the class data buffer.
    fn append_char(&mut self, c: i32);
    /// Whether stack map debug output is enabled.
    fn debug_stackmap(&self) -> bool;
    /// Print stack map debug text.
    fn debug_print(&mut self, text: &str);
    /// Write the verifier type encoding of a local/stack entry.
    fn write_stack_map_type(&mut self, ctx: &mut CompilerContext, t: Option<TypeId>);
}

/// One compressed StackMapTable entry.
#[derive(Clone, Debug)]
pub enum StackMapTableFrame {
    /// frame_type 0..63 or 251 (same frame).
    Same {
        /// The offset delta of the frame.
        offset_delta: i32,
    },
    /// frame_type 64..246 or 247 (same locals, one stack item).
    SameLocals1StackItem {
        /// The offset delta of the frame.
        offset_delta: i32,
        /// The single stack item type.
        stack: TypeId,
    },
    /// frame_type 248..250 (chop frame).
    Chop {
        /// The frame type to emit.
        frame_type: i32,
        /// The offset delta of the frame.
        offset_delta: i32,
    },
    /// frame_type 252..254 (append frame).
    Append {
        /// The frame type to emit.
        frame_type: i32,
        /// The offset delta of the frame.
        offset_delta: i32,
        /// The appended local variable types (None marks an undefined slot, as
        /// upstream Type[] entries may be null; ClassWriter.java:1401-1408).
        locals: Vec<Option<TypeId>>,
    },
    /// frame_type 255 (full frame).
    Full {
        /// The offset delta of the frame.
        offset_delta: i32,
        /// The local variable types.
        locals: Vec<Option<TypeId>>,
        /// The stack types.
        stack: Vec<TypeId>,
    },
}

impl StackMapTableFrame {
    /// Return the frame type of this entry.
    pub fn get_frame_type(&self) -> i32 {
        match self {
            StackMapTableFrame::Same { offset_delta } => {
                if *offset_delta < SAME_FRAME_SIZE {
                    *offset_delta
                } else {
                    SAME_FRAME_EXTENDED
                }
            }
            StackMapTableFrame::SameLocals1StackItem { offset_delta, .. } => {
                if *offset_delta < SAME_FRAME_SIZE {
                    SAME_FRAME_SIZE + *offset_delta
                } else {
                    SAME_LOCALS_1_STACK_ITEM_EXTENDED
                }
            }
            StackMapTableFrame::Chop { frame_type, .. }
            | StackMapTableFrame::Append { frame_type, .. } => *frame_type,
            StackMapTableFrame::Full { .. } => FULL_FRAME,
        }
    }

    /// Write this frame into the writer's data buffer.
    pub fn write(&self, ctx: &mut CompilerContext, writer: &mut dyn StackMapTableWriter) {
        let frame_type = self.get_frame_type();
        writer.append_byte(frame_type);
        if writer.debug_stackmap() {
            writer.debug_print(&format!(" frame_type={}", frame_type));
        }
        match self {
            StackMapTableFrame::Same { offset_delta } => {
                if self.get_frame_type() == SAME_FRAME_EXTENDED {
                    writer.append_char(*offset_delta);
                    if writer.debug_stackmap() {
                        writer.debug_print(&format!(" offset_delta={}", offset_delta));
                    }
                }
            }
            StackMapTableFrame::SameLocals1StackItem {
                offset_delta, stack, ..
            } => {
                if self.get_frame_type() == SAME_LOCALS_1_STACK_ITEM_EXTENDED {
                    writer.append_char(*offset_delta);
                    if writer.debug_stackmap() {
                        writer.debug_print(&format!(" offset_delta={}", offset_delta));
                    }
                }
                if writer.debug_stackmap() {
                    writer.debug_print(&format!(" stack[{}]=", 0));
                }
                writer.write_stack_map_type(ctx, Some(*stack));
            }
            StackMapTableFrame::Chop { offset_delta, .. } => {
                writer.append_char(*offset_delta);
                if writer.debug_stackmap() {
                    writer.debug_print(&format!(" offset_delta={}", offset_delta));
                }
            }
            StackMapTableFrame::Append {
                offset_delta,
                locals,
                ..
            } => {
                writer.append_char(*offset_delta);
                if writer.debug_stackmap() {
                    writer.debug_print(&format!(" offset_delta={}", offset_delta));
                }
                for (i, local) in locals.iter().enumerate() {
                    if writer.debug_stackmap() {
                        writer.debug_print(&format!(" locals[{}]=", i));
                    }
                    writer.write_stack_map_type(ctx, *local);
                }
            }
            StackMapTableFrame::Full {
                offset_delta,
                locals,
                stack,
            } => {
                writer.append_char(*offset_delta);
                writer.append_char(locals.len() as i32);
                if writer.debug_stackmap() {
                    writer.debug_print(&format!(" offset_delta={}", offset_delta));
                    writer.debug_print(&format!(" nlocals={}", locals.len()));
                }
                for (i, local) in locals.iter().enumerate() {
                    if writer.debug_stackmap() {
                        writer.debug_print(&format!(" locals[{}]=", i));
                    }
                    writer.write_stack_map_type(ctx, *local);
                }
                writer.append_char(stack.len() as i32);
                if writer.debug_stackmap() {
                    writer.debug_print(&format!(" nstack={}", stack.len()));
                }
                for (i, item) in stack.iter().enumerate() {
                    if writer.debug_stackmap() {
                        writer.debug_print(&format!(" stack[{}]=", i));
                    }
                    writer.write_stack_map_type(ctx, Some(*item));
                }
            }
        }
    }

    /// Compare a frame with the previous frame and build the compressed entry.
    ///
    /// # Arguments
    ///   - ctx: the compiler context used for type equality;
    ///   - frame: the frame to compress;
    ///   - prev_pc: the code position of the previous frame;
    ///   - prev_locals: the locals of the previous frame;
    ///   - types: the type relations service.
    ///
    /// # Returns
    ///   - the compressed StackMapTable entry.
    ///
    /// # Panics
    ///   - none.
    pub fn get_instance(
        ctx: &mut CompilerContext,
        frame: &StackMapFrame,
        prev_pc: i32,
        prev_locals: &[Option<TypeId>],
        types: &Types,
        syms: &Symtab,
    ) -> StackMapTableFrame {
        let locals = &frame.locals;
        let stack = &frame.stack;
        let offset_delta = frame.pc - prev_pc - 1;
        if stack.len() == 1
            && locals.len() == prev_locals.len()
            && compare(ctx, prev_locals, locals, types, syms) == 0
        {
            return StackMapTableFrame::SameLocals1StackItem {
                offset_delta,
                stack: stack[0],
            };
        }
        if stack.is_empty() {
            let diff_length = compare(ctx, prev_locals, locals, types, syms);
            if diff_length == 0 {
                return StackMapTableFrame::Same { offset_delta };
            }
            if (-MAX_LOCAL_LENGTH_DIFF..0).contains(&diff_length) {
                let mut local_diff = Vec::new();
                let mut i = prev_locals.len();
                while i < locals.len() {
                    local_diff.push(locals[i]);
                    i += 1;
                }
                return StackMapTableFrame::Append {
                    frame_type: SAME_FRAME_EXTENDED - diff_length,
                    offset_delta,
                    locals: local_diff,
                };
            }
            if (0..MAX_LOCAL_LENGTH_DIFF).contains(&diff_length) {
                return StackMapTableFrame::Chop {
                    frame_type: SAME_FRAME_EXTENDED - diff_length,
                    offset_delta,
                };
            }
        }
        StackMapTableFrame::Full {
            offset_delta,
            locals: locals.clone(),
            stack: stack.clone(),
        }
    }
}

/// Return whether a type is represented as an int by the verifier.
fn is_int(ctx: &CompilerContext, t: TypeId) -> bool {
    let tag = ctx.typ(t).get_tag();
    tag.is_strict_sub_range_of(TypeTag::INT) || ctx.typ(t).has_tag(TypeTag::BOOLEAN)
}

/// Compare two verifier types under the StackMapTable equality rules.
///
/// The comparison treats int-compatible types as equal and compares
/// uninitialized pseudo-types by their allocation offset.
pub fn is_same_type(
    ctx: &mut CompilerContext,
    t1: Option<TypeId>,
    t2: Option<TypeId>,
    types: &Types,
    _syms: &Symtab,
) -> bool {
    let (Some(t1), Some(t2)) = (t1, t2) else {
        return t1.is_none() && t2.is_none();
    };
    if is_int(ctx, t1) && is_int(ctx, t2) {
        return true;
    }
    let tag1 = ctx.typ(t1).get_tag();
    let tag2 = ctx.typ(t2).get_tag();
    if tag1 == TypeTag::UNINITIALIZED_THIS {
        return tag2 == TypeTag::UNINITIALIZED_THIS;
    }
    if tag1 == TypeTag::UNINITIALIZED_OBJECT {
        if tag2 == TypeTag::UNINITIALIZED_OBJECT {
            return crate::jvm::uninitialized_type::offset(ctx, t1)
                == crate::jvm::uninitialized_type::offset(ctx, t2);
        }
        return false;
    }
    if tag2 == TypeTag::UNINITIALIZED_THIS || tag2 == TypeTag::UNINITIALIZED_OBJECT {
        return false;
    }
    types.is_same_type(ctx, _syms, t1, t2)
}

/// Compare two local variable vectors and return their length difference, or
/// i32::MAX when they differ beyond the compact limit.
pub fn compare(
    ctx: &mut CompilerContext,
    arr1: &[Option<TypeId>],
    arr2: &[Option<TypeId>],
    types: &Types,
    syms: &Symtab,
) -> i32 {
    let diff_length = arr1.len() as i32 - arr2.len() as i32;
    if !(-MAX_LOCAL_LENGTH_DIFF..=MAX_LOCAL_LENGTH_DIFF).contains(&diff_length) {
        return i32::MAX;
    }
    let len = if diff_length > 0 { arr2.len() } else { arr1.len() };
    for i in 0..len {
        if !is_same_type(ctx, arr1[i], arr2[i], types, syms) {
            return i32::MAX;
        }
    }
    diff_length
}
