//! Shared constant-pool state: the writeable pool helper and the signature
//! generator used by the pool writer.

use std::collections::VecDeque;
use std::rc::Rc;

use crate::code::type_tag::TypeTag;
use crate::code::types::{InvalidSignatureException, SignatureGenerator, Types};
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TypeId};
use crate::jvm::pool_constant::PoolKey;
use crate::jvm::pool_writer::{PoolConstantRef, PoolWriter};
use crate::util::byte_buffer::ByteBuffer;
use crate::util::name::Name;

/// Java: PoolWriter.enterInner (PoolWriter.java:223-231): enter the class and
/// its enclosing classes only when the class is nested (its owner has an
/// enclosing class); top-level classes are not entered.
pub(crate) fn enter_inner_symbol(
    ctx: &mut CompilerContext,
    types: &crate::code::types::Types,
    inner_classes: &mut Vec<SymbolId>,
    c: SymbolId,
) {
    let c_ty = ctx.sym(c).ty.expect("class symbol type");
    if types.is_compound_public(ctx, c_ty) {
        panic!("AssertionError: Unexpected intersection type");
    }
    if inner_classes.contains(&c) {
        return;
    }
    if let Some(owner) = ctx.sym(c).owner
        && let Some(encl) = crate::code::symbol::symbol_encl_class(ctx, owner)
    {
        enter_inner_symbol(ctx, types, inner_classes, encl);
        inner_classes.push(c);
    }
}

/// The writeable pool helper state.
pub struct WriteablePoolHelper {
    /// Pool entry keys with their positions.
    pub keys_to_pos: Vec<(PoolKey, i32)>,
    /// The constant pool byte buffer.
    pub poolbuf: ByteBuffer,
    /// The next free pool index.
    pub current_index: i32,
    /// Pending entries to write.
    pub todo: VecDeque<PoolConstantRef>,
    /// The first overflowing string, if any.
    pub overflow_string: Option<String>,
}

impl WriteablePoolHelper {
    /// Create the empty helper state.
    pub(crate) fn new() -> WriteablePoolHelper {
        WriteablePoolHelper {
            keys_to_pos: Vec::new(),
            poolbuf: ByteBuffer::with_initial_size(PoolWriter::POOL_BUF_SIZE),
            current_index: 1,
            todo: VecDeque::new(),
            overflow_string: None,
        }
    }

    /// Reset the helper state.
    pub(crate) fn reset(&mut self) {
        self.keys_to_pos.clear();
        self.current_index = 1;
        self.todo.clear();
        self.overflow_string = None;
        self.poolbuf.reset();
    }
}

/// The shared signature generator used by the pool writer.
pub struct SharedSignatureGenerator {
    /// The signature output buffer.
    pub sigbuf: ByteBuffer,
    /// Type relations.
    pub types: Rc<Types>,
    /// Inner classes collected through class references.
    pub inner_classes: Vec<SymbolId>,
}

impl SharedSignatureGenerator {
    /// Create the generator.
    pub(crate) fn new(types: Rc<Types>) -> SharedSignatureGenerator {
        SharedSignatureGenerator {
            sigbuf: ByteBuffer::new(),
            types,
            inner_classes: Vec::new(),
        }
    }

    /// Reset the output buffer.
    pub fn reset(&mut self) {
        self.sigbuf.reset();
    }

    /// Return the assembled name.
    pub fn to_name(&self, names: &crate::util::names::Names) -> Name {
        self.sigbuf.to_name(names)
    }

    /// Move the collected signature references into the writer list.
    ///
    /// Java: SignatureGenerator.classReference calls PoolWriter.enterInner
    /// (PoolWriter.java:318-319), which only enters classes with an enclosing
    /// class (PoolWriter.java:223-231). The shared generator has no context,
    /// so the official enterInner semantics are applied here, at the merge
    /// into PoolWriter.inner_classes.
    pub(crate) fn take_inner_classes_into(
        &mut self,
        ctx: &mut CompilerContext,
        inner_classes: &mut Vec<SymbolId>,
    ) {
        for c in std::mem::take(&mut self.inner_classes) {
            enter_inner_symbol(ctx, &self.types, inner_classes, c);
        }
    }
}

impl SignatureGenerator for SharedSignatureGenerator {
    fn append_char(&mut self, ch: char) {
        self.sigbuf.append_byte(ch as i32);
    }

    fn append_bytes(&mut self, ba: &[u8]) {
        self.sigbuf.append_bytes(ba);
    }

    fn append_name(&mut self, name: &Name) {
        self.sigbuf.append_name(name);
    }

    /// Java: SignatureGenerator.classReference (PoolWriter.java:318-319)
    /// calls PoolWriter.enterInner. The shared generator has no writer or
    /// context, so it records the referenced class here; the enterInner
    /// condition is applied when the candidates are merged into
    /// PoolWriter.inner_classes (take_inner_classes_into).
    fn class_reference(&mut self, c: SymbolId) {
        if !self.inner_classes.contains(&c) {
            self.inner_classes.push(c);
        }
    }

    fn types(&self) -> &Types {
        &self.types
    }

    fn assemble_sig(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        t: TypeId,
    ) -> Result<(), InvalidSignatureException> {
        match ctx.typ(t).get_tag() {
            TypeTag::UNINITIALIZED_THIS | TypeTag::UNINITIALIZED_OBJECT => {
                let qtype = crate::jvm::uninitialized_type::initialized_type(ctx, t)
                    .expect("uninitialized pseudo-type qtype");
                let erased = self.types.erasure(ctx, syms, qtype);
                self.assemble_sig_default(ctx, syms, erased)
            }
            _ => self.assemble_sig_default(ctx, syms, t),
        }
    }
}
