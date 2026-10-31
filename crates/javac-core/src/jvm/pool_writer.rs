// Source: com/sun/tools/javac/jvm/PoolWriter.java

//! Constant pool writer: turns class file entities (names, members, dynamic
//! constants) into the constant pool byte buffer and the BootstrapMethods table.

use std::collections::VecDeque;
use std::rc::Rc;

use crate::code::type_tag::TypeTag;
use crate::code::types::{InvalidSignatureException, SignatureGenerator, Types};
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TypeId};
use crate::jvm::class_file;
use crate::jvm::pool_constant::{
    BasicConstant, BsmKey, LoadableConstantRef, NameAndType, PoolConstant, PoolKey,
};
use crate::util::byte_buffer::ByteBuffer;
use crate::util::name::Name;
use crate::util::names::Names;

/// Raised when the constant pool exceeds the JVMS entry count limit.
#[derive(Clone, Debug)]
pub struct PoolOverflow;

/// Raised when a Utf8 string constant exceeds the JVMS length limit.
#[derive(Clone, Debug)]
pub struct StringOverflow {
    /// The first overflowing string.
    pub overflow_string: String,
}

/// Error raised while writing the constant pool.
#[derive(Clone, Debug)]
pub enum PoolWriterError {
    /// The pool has too many entries.
    PoolOverflow,
    /// A string constant is too long.
    StringOverflow(String),
}

/// One constant pool entry producer.
#[derive(Clone, Debug)]
pub enum PoolConstantRef {
    /// A Utf8 entry.
    Utf8(Name),
    /// A class entry.
    Class(TypeId),
    /// A field or method reference entry.
    Member(SymbolId),
    /// A package entry.
    Package(SymbolId),
    /// A module entry.
    Module(SymbolId),
    /// A basic constant entry.
    Basic(BasicConstant),
    /// A method handle entry.
    MethodHandle(SymbolId),
    /// A method type entry.
    MethodType(TypeId),
    /// A name-and-type entry.
    NameAndType(NameAndType),
    /// A dynamic entry (invokedynamic or constant dynamic).
    Dynamic(SymbolId),
}

/// The constant pool writer shared by the code generator and the class writer.
pub struct PoolWriter {
    /// Type relations.
    pub types: Rc<Types>,
    /// The name table.
    pub names: Rc<Names>,
    /// The writeable pool helper state.
    pub pool: WriteablePoolHelper,
    /// The shared signature generator.
    pub signature_gen: SharedSignatureGenerator,
    /// Inner classes to write, enclosing first.
    pub inner_classes: Vec<SymbolId>,
    /// BootstrapMethods entries in first-use order.
    pub bootstrap_methods: Vec<(BsmKey, i32)>,
    /// The offending type when the last signature could not be assembled.
    pub signature_error: Option<TypeId>,
}

/// Java: PoolWriter.enterInner (PoolWriter.java:223-231): enter the class and
/// its enclosing classes only when the class is nested (its owner has an
/// enclosing class); top-level classes are not entered.
fn enter_inner_symbol(
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

impl PoolWriter {
    /// Maximum number of constant pool entries.
    pub const MAX_ENTRIES: i32 = 0xFFFF;
    /// Maximum Utf8 constant length.
    pub const MAX_STRING_LENGTH: i32 = 0xFFFF;
    /// Initial pool buffer size.
    const POOL_BUF_SIZE: i32 = 0x7fff;

    /// Create a pool writer for one class.
    pub fn new(types: Rc<Types>, names: Rc<Names>) -> PoolWriter {
        PoolWriter {
            signature_gen: SharedSignatureGenerator::new(Rc::clone(&types)),
            types,
            names,
            pool: WriteablePoolHelper::new(),
            inner_classes: Vec::new(),
            bootstrap_methods: Vec::new(),
            signature_error: None,
        }
    }

    /// Return the current pool size.
    pub fn size(&self) -> i32 {
        self.pool.current_index
    }

    /// Reset the writer for the next class.
    pub fn reset(&mut self) {
        self.inner_classes.clear();
        self.bootstrap_methods.clear();
        self.signature_error = None;
        self.pool.reset();
    }

    /// Take the offending signature type, if the class could not be written.
    pub fn take_signature_error(&mut self) -> Option<TypeId> {
        self.signature_error.take()
    }

    /// Put a class symbol into the pool.
    pub fn put_class_symbol(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        csym: SymbolId,
    ) -> i32 {
        let ty = ctx.sym(csym).ty.expect("class symbol type");
        self.put_class(ctx, syms, ty)
    }

    /// Put a class type into the pool.
    pub fn put_class(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        t: TypeId,
    ) -> i32 {
        let erased = self.types.erasure(ctx, syms, t);
        self.write_if_needed(ctx, syms, PoolConstantRef::Class(erased))
    }

    /// Put a member reference into the pool.
    pub fn put_member(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        s: SymbolId,
    ) -> i32 {
        self.write_if_needed(ctx, syms, PoolConstantRef::Member(s))
    }

    /// Put a dynamic reference into the pool.
    pub fn put_dynamic(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        d: SymbolId,
    ) -> i32 {
        self.write_if_needed(ctx, syms, PoolConstantRef::Dynamic(d))
    }

    /// Put a descriptor type into the pool.
    pub fn put_descriptor(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        t: TypeId,
    ) -> i32 {
        let erased = self.types.erasure(ctx, syms, t);
        let name = self.type_sig(ctx, syms, erased);
        self.write_if_needed(ctx, syms, PoolConstantRef::Utf8(name))
    }

    /// Put a symbol descriptor into the pool.
    pub fn put_descriptor_symbol(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        s: SymbolId,
    ) -> i32 {
        let descriptor = self.descriptor_type(ctx, syms, s);
        self.put_descriptor(ctx, syms, descriptor)
    }

    /// Put a signature attribute value into the pool.
    pub fn put_signature(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        s: SymbolId,
    ) -> i32 {
        let name = if ctx.sym(s).kind == crate::code::kinds::Kind::TYP {
            let ty = ctx.sym(s).ty.expect("type symbol type");
            self.class_sig(ctx, syms, ty)
        } else {
            let ty = ctx.sym(s).ty.expect("symbol type");
            self.type_sig(ctx, syms, ty)
        };
        // Java: SignatureGenerator.classReference calls PoolWriter.enterInner
        // (PoolWriter.java:318-319, 223-231); the shared generator defers the
        // class references, so merge them whenever a signature is put.
        self.signature_gen.take_inner_classes_into(ctx, &mut self.inner_classes);
        self.put_name(name)
    }

    /// Put a constant value into the pool.
    pub fn put_constant_value(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        value: &crate::util::constants::ConstantValue,
    ) -> i32 {
        use crate::util::constants::ConstantValue;
        let constant = match value {
            ConstantValue::Integer(v) => BasicConstant::int(*v),
            ConstantValue::Float(v) => BasicConstant::float(*v),
            ConstantValue::Long(v) => BasicConstant::long(*v),
            ConstantValue::Double(v) => BasicConstant::double(*v),
            ConstantValue::String(v) => BasicConstant::string(v.clone()),
            // The JVM pool stores byte/short/char/boolean as a CONSTANT_Integer
            // (JVMS 4.4.4); the Java primitive wrapper values are widened here.
            ConstantValue::Byte(v) => BasicConstant::int(*v as i32),
            ConstantValue::Short(v) => BasicConstant::int(*v as i32),
            ConstantValue::Character(v) => BasicConstant::int(*v as i32),
            ConstantValue::Boolean(v) => BasicConstant::int(i32::from(*v)),
        };
        self.put_constant(ctx, syms, PoolConstantRef::Basic(constant))
    }

    /// Put a loadable constant into the pool.
    pub fn put_constant(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        constant: PoolConstantRef,
    ) -> i32 {
        let tag = pool_tag_of(&constant);
        match tag {
            class_file::CONSTANT_CLASS => {
                let ty = match constant {
                    PoolConstantRef::Class(ty) => ty,
                    _ => panic!("AssertionError: class constant expected"),
                };
                self.put_class(ctx, syms, ty)
            }
            class_file::CONSTANT_METHODTYPE => {
                let ty = match constant {
                    PoolConstantRef::MethodType(ty) => ty,
                    _ => panic!("AssertionError: method type constant expected"),
                };
                let erased = self.types.erasure(ctx, syms, ty);
                self.write_if_needed(ctx, syms, PoolConstantRef::MethodType(erased))
            }
            _ => self.write_if_needed(ctx, syms, constant),
        }
    }

    /// Put a Utf8 name into the pool.
    pub fn put_name(&mut self, name: Name) -> i32 {
        self.write_plain(PoolConstantRef::Utf8(name))
    }

    /// Put a name-and-type entry of a symbol into the pool.
    pub fn put_name_and_type(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        s: SymbolId,
    ) -> i32 {
        let name = ctx.sym(s).name.clone();
        let descriptor = self.descriptor_type(ctx, syms, s);
        let nt = NameAndType::new(name, descriptor);
        self.write_if_needed(ctx, syms, PoolConstantRef::NameAndType(nt))
    }

    /// Put a package entry into the pool.
    pub fn put_package(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        pkg: SymbolId,
    ) -> i32 {
        self.write_if_needed(ctx, syms, PoolConstantRef::Package(pkg))
    }

    /// Put a module entry into the pool.
    pub fn put_module(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        module: SymbolId,
    ) -> i32 {
        self.write_if_needed(ctx, syms, PoolConstantRef::Module(module))
    }

    /// Java: public void enterInner(ClassSymbol c) (PoolWriter.java:223-231).
    pub fn enter_inner(&mut self, ctx: &mut CompilerContext, c: SymbolId) {
        enter_inner_symbol(ctx, &self.types, &mut self.inner_classes, c);
    }

    /// Write the pool into the given class-file byte buffer.
    pub fn write_pool(&self, out: &mut Vec<u8>) -> Result<(), PoolWriterError> {
        if let Some(overflow) = &self.pool.overflow_string {
            return Err(PoolWriterError::StringOverflow(overflow.clone()));
        }
        let size = self.size();
        if size > Self::MAX_ENTRIES {
            return Err(PoolWriterError::PoolOverflow);
        }
        out.push((size >> 8) as u8);
        out.push(size as u8);
        out.extend_from_slice(&self.pool.poolbuf.elems[..self.pool.poolbuf.length as usize]);
        Ok(())
    }

    /// Return the descriptor type of a member symbol.
    fn descriptor_type(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        s: SymbolId,
    ) -> TypeId {
        let ty = ctx.sym(s).ty.expect("symbol type");
        if ctx.sym(s).kind == crate::code::kinds::Kind::MTH {
            crate::code::symbol::external_type(ctx, &self.types, syms, &self.names, s)
        } else {
            self.types.erasure(ctx, syms, ty)
        }
    }

    /// Return the pool key of an entry.
    fn key_of(&self, ctx: &mut CompilerContext, constant: &PoolConstantRef) -> PoolKey {
        match constant {
            PoolConstantRef::Utf8(name) => PoolKey::Utf8(name.clone()),
            PoolConstantRef::Class(ty) => PoolKey::Class(*ty),
            PoolConstantRef::Member(s) => crate::code::symbol::symbol_pool_key(ctx, *s),
            PoolConstantRef::Package(s) | PoolConstantRef::Module(s) => PoolKey::Symbol(*s),
            PoolConstantRef::Basic(basic) => basic.pool_key(),
            PoolConstantRef::MethodHandle(s) => method_handle_pool_key(&*ctx, *s),
            PoolConstantRef::MethodType(ty) => PoolKey::Type(*ty),
            PoolConstantRef::NameAndType(nt) => PoolKey::NameAndType(nt.name.clone(), nt.type_),
            PoolConstantRef::Dynamic(s) => PoolKey::Dynamic(*s),
        }
    }

    /// Write a constant when it is new and return its pool index.
    fn write_if_needed(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        constant: PoolConstantRef,
    ) -> i32 {
        let key = self.key_of(ctx, &constant);
        for (existing, index) in &self.pool.keys_to_pos {
            if pool_keys_equal(ctx, syms, &self.types, existing, &key) {
                return *index;
            }
        }
        let index = self.pool.current_index;
        self.pool.current_index += 1;
        self.pool.keys_to_pos.push((key, index));
        let first = self.pool.todo.is_empty();
        self.pool.todo.push_back(constant);
        if first {
            while !self.pool.todo.is_empty() {
                let front = self.pool.todo.front().expect("pending constant").clone();
                self.write_constant(ctx, syms, front);
                self.pool.todo.pop_front();
            }
        }
        index
    }

    /// Write a plain Utf8 entry.
    fn write_plain(&mut self, constant: PoolConstantRef) -> i32 {
        let key = match &constant {
            PoolConstantRef::Utf8(name) => PoolKey::Utf8(name.clone()),
            _ => panic!("AssertionError: plain constant expected"),
        };
        for (existing, index) in &self.pool.keys_to_pos {
            if existing == &key {
                return *index;
            }
        }
        let index = self.pool.current_index;
        self.pool.current_index += 1;
        self.pool.keys_to_pos.push((key, index));
        self.write_constant_utf8(constant);
        index
    }

    /// Write a Utf8 constant entry.
    fn write_constant_utf8(&mut self, constant: PoolConstantRef) {
        let name = match constant {
            PoolConstantRef::Utf8(name) => name,
            _ => panic!("AssertionError: Utf8 constant expected"),
        };
        self.pool.poolbuf.append_byte(class_file::CONSTANT_UTF8);
        let bytes = name.to_utf();
        self.pool.poolbuf.append_char(bytes.len() as i32);
        self.pool
            .poolbuf
            .append_bytes_from(&bytes, 0, bytes.len() as i32);
        if self.pool.overflow_string.is_none()
            && bytes.len() as i32 > Self::MAX_STRING_LENGTH
        {
            self.pool.overflow_string = Some(String::from_utf8_lossy(&bytes).to_string());
        }
    }

    /// Write one constant entry, adding dependent entries as needed.
    fn write_constant(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        constant: PoolConstantRef,
    ) {
        match constant {
            PoolConstantRef::Utf8(_) => self.write_constant_utf8(constant),
            PoolConstantRef::Class(ty) => {
                self.pool.poolbuf.append_byte(class_file::CONSTANT_CLASS);
                let name_index = if ctx.typ(ty).has_tag(TypeTag::ARRAY) {
                    let name = self.type_sig(ctx, syms, ty);
                    self.write_if_needed(ctx, syms, PoolConstantRef::Utf8(name))
                } else {
                    let tsym = ctx.typ(ty).tsym.expect("class type symbol");
                    let flat = ctx.sym(tsym).flat_name();
                    let external = class_file::externalize_name(&flat);
                    let name = self.names.from_utf(&external);
                    self.write_if_needed(ctx, syms, PoolConstantRef::Utf8(name))
                };
                self.pool.poolbuf.append_char(name_index);
                if ctx.typ(ty).has_tag(TypeTag::CLASS)
                    && let Some(tsym) = ctx.typ(ty).tsym
                {
                    self.enter_inner(ctx, tsym);
                }
            }
            PoolConstantRef::Member(sym) => {
                let tag = crate::code::symbol::symbol_pool_tag(ctx, sym);
                self.pool.poolbuf.append_byte(tag);
                let owner = ctx.sym(sym).owner.expect("member owner");
                let owner_index = {
                    let owner_ty = ctx.sym(owner).ty.expect("member owner type");
                    let erased = self.types.erasure(ctx, syms, owner_ty);
                    self.put_class(ctx, syms, erased)
                };
                self.pool.poolbuf.append_char(owner_index);
                let name = ctx.sym(sym).name.clone();
                let descriptor = self.descriptor_type(ctx, syms, sym);
                let nt = NameAndType::new(name, descriptor);
                let nt_index = self.write_if_needed(ctx, syms, PoolConstantRef::NameAndType(nt));
                self.pool.poolbuf.append_char(nt_index);
            }
            PoolConstantRef::Package(pkg) => {
                let flat = ctx.sym(pkg).flat_name();
                let external = class_file::externalize_name(&flat);
                let name = self.names.from_utf(&external);
                self.pool.poolbuf.append_byte(class_file::CONSTANT_PACKAGE);
                let name_index = self.write_if_needed(ctx, syms, PoolConstantRef::Utf8(name));
                self.pool.poolbuf.append_char(name_index);
            }
            PoolConstantRef::Module(module) => {
                self.pool.poolbuf.append_byte(class_file::CONSTANT_MODULE);
                let name = ctx.sym(module).name.clone();
                let mod_name = self.write_if_needed(ctx, syms, PoolConstantRef::Utf8(name));
                self.pool.poolbuf.append_char(mod_name);
            }
            PoolConstantRef::Basic(basic) => {
                self.pool.poolbuf.append_byte(basic.pool_tag());
                match basic.pool_key() {
                    PoolKey::Integer(v) => self.pool.poolbuf.append_int(v),
                    PoolKey::Float(v) => self.pool.poolbuf.append_float(f32::from_bits(v)),
                    PoolKey::Long(v) => {
                        self.pool.current_index += 1;
                        self.pool.poolbuf.append_long(v);
                    }
                    PoolKey::Double(v) => {
                        self.pool.current_index += 1;
                        self.pool.poolbuf.append_double(f64::from_bits(v));
                    }
                    PoolKey::String(text) => {
                        let name = self.names.from_string(&text);
                        let name_index = self.write_if_needed(ctx, syms, PoolConstantRef::Utf8(name));
                        self.pool.poolbuf.append_char(name_index);
                    }
                    _ => panic!("AssertionError: unexpected basic constant"),
                }
            }
            PoolConstantRef::MethodHandle(handle) => {
                let reference_kind = crate::code::symbol::method_handle_reference_kind(ctx, handle)
                    .expect("method handle reference kind");
                self.pool.poolbuf.append_byte(class_file::CONSTANT_METHODHANDLE);
                self.pool.poolbuf.append_byte(reference_kind);
                let base = crate::code::symbol::method_handle_base_symbol(ctx, handle)
                    .expect("method handle base symbol");
                let member_index = self.put_member(ctx, syms, base);
                self.pool.poolbuf.append_char(member_index);
            }
            PoolConstantRef::MethodType(ty) => {
                self.pool.poolbuf.append_byte(class_file::CONSTANT_METHODTYPE);
                let base = self.types.base_type_of(ctx, syms, ty);
                let descriptor = self.put_descriptor(ctx, syms, base);
                self.pool.poolbuf.append_char(descriptor);
            }
            PoolConstantRef::NameAndType(nt) => {
                self.pool.poolbuf.append_byte(class_file::CONSTANT_NAMEANDTYPE);
                let name_index = self.write_if_needed(ctx, syms, PoolConstantRef::Utf8(nt.name.clone()));
                self.pool.poolbuf.append_char(name_index);
                let descriptor = self.put_descriptor(ctx, syms, nt.type_);
                self.pool.poolbuf.append_char(descriptor);
            }
            PoolConstantRef::Dynamic(dynamic) => {
                let tag = crate::code::symbol::symbol_pool_tag(ctx, dynamic);
                self.pool.poolbuf.append_byte(tag);
                let bsm_index = self.make_bootstrap_entry(ctx, dynamic);
                self.pool.poolbuf.append_char(bsm_index);
                let name = ctx.sym(dynamic).name.clone();
                let ty = ctx.sym(dynamic).ty.expect("dynamic symbol type");
                let descriptor = self.types.erasure(ctx, syms, ty);
                let nt = NameAndType::new(name, descriptor);
                let nt_index = self.write_if_needed(ctx, syms, PoolConstantRef::NameAndType(nt));
                self.pool.poolbuf.append_char(nt_index);
            }
        }
    }

    /// Return the BootstrapMethods index of a dynamic symbol.
    fn make_bootstrap_entry(&mut self, ctx: &mut CompilerContext, dynamic: SymbolId) -> i32 {
        let bsm = crate::code::symbol::dynamic_bootstrap_method(ctx, dynamic)
            .expect("dynamic symbol bootstrap method");
        let static_args = crate::code::symbol::dynamic_static_args(ctx, dynamic)
            .expect("dynamic symbol static args");
        let static_arg_keys: Vec<PoolKey> = static_args
            .iter()
            .map(|arg| loadable_pool_key(&*ctx, arg))
            .collect();
        let key = BsmKey {
            bsm: Some(bsm),
            static_args,
            bsm_key: method_handle_pool_key(&*ctx, bsm),
            static_arg_keys,
        };
        for (existing, index) in &self.bootstrap_methods {
            if bsm_keys_equal(existing, &key) {
                return *index;
            }
        }
        let index = self.bootstrap_methods.len() as i32;
        self.bootstrap_methods.push((key, index));
        index
    }

    /// Return the type signature name.
    fn type_sig(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        ty: TypeId,
    ) -> Name {
        self.signature_gen.reset();
        if let Err(error) = self.signature_gen.assemble_sig(ctx, syms, ty) {
            if self.signature_error.is_none() {
                self.signature_error = Some(error.type_);
            }
            return self.names.empty.clone();
        }
        let names = Rc::clone(&self.names);
        self.signature_gen.take_inner_classes_into(ctx, &mut self.inner_classes);
        self.signature_gen.to_name(&names)
    }

    /// Return the class signature name.
    fn class_sig(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        t: TypeId,
    ) -> Name {
        self.signature_gen.reset();
        let typarams =
            crate::code::types::get_type_arguments_completed(ctx, syms, t, None);
        if !typarams.is_empty()
            && let Err(error) = self.signature_gen.assemble_params_sig(ctx, syms, typarams)
        {
            if self.signature_error.is_none() {
                self.signature_error = Some(error.type_);
            }
            return self.names.empty.clone();
        }
        let super_ty = self.types.supertype(ctx, syms, t, None);
        if let Err(error) = self.signature_gen.assemble_sig(ctx, syms, super_ty) {
            if self.signature_error.is_none() {
                self.signature_error = Some(error.type_);
            }
            return self.names.empty.clone();
        }
        for interface in self.types.interfaces(ctx, syms, t).iter() {
            if let Err(error) = self.signature_gen.assemble_sig(ctx, syms, interface) {
                if self.signature_error.is_none() {
                    self.signature_error = Some(error.type_);
                }
                return self.names.empty.clone();
            }
        }
        let names = Rc::clone(&self.names);
        self.signature_gen.take_inner_classes_into(ctx, &mut self.inner_classes);
        self.signature_gen.to_name(&names)
    }
}

/// Return the pool tag of a constant reference.
fn pool_tag_of(constant: &PoolConstantRef) -> i32 {
    match constant {
        PoolConstantRef::Utf8(_) => class_file::CONSTANT_UTF8,
        PoolConstantRef::Class(_) => class_file::CONSTANT_CLASS,
        PoolConstantRef::Member(s) => {
            let _ = s;
            class_file::CONSTANT_FIELDREF
        }
        PoolConstantRef::Package(_) => class_file::CONSTANT_PACKAGE,
        PoolConstantRef::Module(_) => class_file::CONSTANT_MODULE,
        PoolConstantRef::Basic(basic) => basic.pool_tag(),
        PoolConstantRef::MethodHandle(_) => class_file::CONSTANT_METHODHANDLE,
        PoolConstantRef::MethodType(_) => class_file::CONSTANT_METHODTYPE,
        PoolConstantRef::NameAndType(_) => class_file::CONSTANT_NAMEANDTYPE,
        PoolConstantRef::Dynamic(s) => {
            let _ = s;
            class_file::CONSTANT_INVOKEDYNAMIC
        }
    }
}

/// Java: MethodHandleSymbol.poolKey (Symbol.java:2403-2406):
/// new Pair<>(baseSymbol(), referenceKind()).
fn method_handle_pool_key(
    ctx: &CompilerContext,
    handle: SymbolId,
) -> PoolKey {
    let base = crate::code::symbol::method_handle_base_symbol(ctx, handle)
        .expect("method handle base symbol");
    let reference_kind = crate::code::symbol::method_handle_reference_kind(ctx, handle)
        .expect("method handle reference kind");
    PoolKey::Pair(
        Box::new(PoolKey::Symbol(base)),
        Box::new(PoolKey::Integer(reference_kind)),
    )
}

/// Return the pool key of a loadable constant reference.
fn loadable_pool_key(
    ctx: &CompilerContext,
    arg: &LoadableConstantRef,
) -> PoolKey {
    match arg {
        LoadableConstantRef::Basic(basic) => basic.pool_key(),
        LoadableConstantRef::Class(ty) => PoolKey::Class(*ty),
        LoadableConstantRef::MethodHandle(sym) => method_handle_pool_key(ctx, *sym),
        LoadableConstantRef::MethodType(ty) => PoolKey::Type(*ty),
        LoadableConstantRef::Dynamic(sym) => PoolKey::Dynamic(*sym),
    }
}

/// Compare two pool keys under the Java constant-pool equality rules.
fn pool_keys_equal(
    ctx: &mut CompilerContext,
    syms: &crate::code::symtab::Symtab,
    types: &Types,
    a: &PoolKey,
    b: &PoolKey,
) -> bool {
    match (a, b) {
        (PoolKey::Type(t1), PoolKey::Type(t2))
        | (PoolKey::Class(t1), PoolKey::Class(t2)) => {
            types.is_same_type(ctx, syms, *t1, *t2)
        }
        (PoolKey::NameAndType(n1, t1), PoolKey::NameAndType(n2, t2)) => {
            n1 == n2 && types.is_same_type(ctx, syms, *t1, *t2)
        }
        (PoolKey::Pair(x1, y1), PoolKey::Pair(x2, y2)) => {
            pool_keys_equal(ctx, syms, types, x1, x2)
                && pool_keys_equal(ctx, syms, types, y1, y2)
        }
        // Java: Dynamic.poolKey (PoolConstant.java:151-154) is
        // Pair(bsmKey, dynamicType().poolKey(types)); equal dynamic symbols
        // share one InvokeDynamic/ConstantDynamic pool entry.
        (PoolKey::Dynamic(s1), PoolKey::Dynamic(s2)) => {
            if s1 == s2 {
                return true;
            }
            let (Some(h1), Some(h2)) = (
                crate::code::symbol::dynamic_bootstrap_method(ctx, *s1),
                crate::code::symbol::dynamic_bootstrap_method(ctx, *s2),
            ) else {
                return false;
            };
            if method_handle_pool_key(&*ctx, h1) != method_handle_pool_key(&*ctx, h2) {
                return false;
            }
            let a1 = crate::code::symbol::dynamic_static_args(ctx, *s1).unwrap_or_default();
            let a2 = crate::code::symbol::dynamic_static_args(ctx, *s2).unwrap_or_default();
            if a1.len() != a2.len() {
                return false;
            }
            for (x, y) in a1.iter().zip(a2.iter()) {
                if !pool_keys_equal(
                    ctx,
                    syms,
                    types,
                    &loadable_pool_key(&*ctx, x),
                    &loadable_pool_key(&*ctx, y),
                ) {
                    return false;
                }
            }
            let t1 = ctx.sym(*s1).ty.expect("dynamic type");
            let t2 = ctx.sym(*s2).ty.expect("dynamic type");
            types.is_same_type(ctx, syms, t1, t2)
        }
        _ => a == b,
    }
}

/// Compare two bootstrap-method keys.
fn bsm_keys_equal(a: &BsmKey, b: &BsmKey) -> bool {
    a.bsm_key == b.bsm_key && a.static_arg_keys == b.static_arg_keys
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
    fn new() -> WriteablePoolHelper {
        WriteablePoolHelper {
            keys_to_pos: Vec::new(),
            poolbuf: ByteBuffer::with_initial_size(PoolWriter::POOL_BUF_SIZE),
            current_index: 1,
            todo: VecDeque::new(),
            overflow_string: None,
        }
    }

    /// Reset the helper state.
    fn reset(&mut self) {
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
    fn new(types: Rc<Types>) -> SharedSignatureGenerator {
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
    pub fn to_name(&self, names: &Names) -> Name {
        self.sigbuf.to_name(names)
    }

    /// Move the collected signature references into the writer list.
    ///
    /// Java: SignatureGenerator.classReference calls PoolWriter.enterInner
    /// (PoolWriter.java:318-319), which only enters classes with an enclosing
    /// class (PoolWriter.java:223-231). The shared generator has no context,
    /// so the official enterInner semantics are applied here, at the merge
    /// into PoolWriter.inner_classes.
    fn take_inner_classes_into(
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
