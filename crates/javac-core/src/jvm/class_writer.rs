// Source: com/sun/tools/javac/jvm/ClassWriter.java

//! Maps the symbol table graph of a class into a class file.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::code::attribute::{AttributeKind, RetentionPolicy};
use crate::code::directive::{Directive, ExportsFlag, OpensFlag, RequiresFlag};
use crate::code::flags;
use crate::code::preview::Preview;
use crate::code::source::Source;
use crate::code::symbol::{Symbol, SymbolKind};
use crate::code::type_::{Type, TypeKindData};
use crate::code::type_annotation_position::TypeAnnotationPosition;
use crate::code::target_type::TargetType;
use crate::code::types::{SignatureGenerator, Types};
use crate::comp::check::Check;
use crate::compiler_context::CompilerContext;
use crate::ids::{AttributeId, SymbolId, TypeId};
use crate::jvm::class_file;
use crate::jvm::code::Code;
use crate::jvm::pool_constant::loadable_to_pool_constant;
use crate::jvm::pool_writer::{PoolWriter, PoolWriterError};
use crate::jvm::stack_map::{
    StackMapFormat, StackMapFrame, StackMapTableFrame, StackMapTableWriter,
};
use crate::jvm::target::Target;
use crate::util::byte_buffer::ByteBuffer;
use crate::util::list::List;
use crate::util::log::Log;
use crate::util::name::Name;
use crate::util::names::Names;
use crate::util::options::Options;

use crate::jvm::class_writer_support::{
    class_flat_name, class_sourcefile, create_module_info_class, current_time_millis,    last_modified, qualified_name, simple_name,};

/// The error raised while writing a class file.
#[derive(Clone, Debug)]
pub enum ClassWriterError {
    /// The constant pool overflowed.
    PoolOverflow,
    /// A string constant is too long.
    StringOverflow(String),
    /// The signature of a class member is illegal (the offending type).
    InvalidSignature(TypeId),
}

/// An extra attribute writer hook.
type ExtraAttributeHook = Box<dyn FnMut(&mut CompilerContext, SymbolId) -> i32>;

/// The flags whose names are printed by the flagNames debug helper.
const STANDARD_FLAGS: i64 = 0x0fff;

/// The names of the standard flags, indexed by bit position.
const FLAG_NAMES: [&str; 12] = [
    "PUBLIC",
    "PRIVATE",
    "PROTECTED",
    "STATIC",
    "FINAL",
    "SUPER",
    "VOLATILE",
    "TRANSIENT",
    "NATIVE",
    "INTERFACE",
    "ABSTRACT",
    "STRICTFP",
];

/// The class file writer.
pub struct ClassWriter {
    /// The diagnostic log.
    pub log: Rc<Log>,
    /// The name table.
    pub names: Rc<Names>,
    /// The symbol table.
    pub syms: Rc<crate::code::symtab::Symtab>,
    /// Type relations.
    pub types: Rc<Types>,
    /// The check service.
    pub check: Rc<Check>,
    /// The compiler options.
    pub options: Rc<Options>,
    /// The code generation target.
    pub target: Target,
    /// The source language level.
    pub source: Option<Source>,
    /// The preview feature helper.
    pub preview: Rc<Preview>,
    /// The class file data buffer.
    pub databuf: RefCell<ByteBuffer>,
    /// The class file constant pool header buffer.
    pub poolbuf: RefCell<ByteBuffer>,
    /// The constant pool writer.
    pub pool_writer: Rc<RefCell<PoolWriter>>,
    /// The code objects of the generated methods, indexed by symbol.
    pub method_codes: Rc<RefCell<HashMap<SymbolId, Rc<RefCell<Code>>>>>,
    /// Whether to write verbose progress output.
    pub verbose: bool,
    /// Whether to emit the SourceFile attribute.
    pub emit_source_file: bool,
    /// Whether the character range table is generated.
    pub gen_crt: bool,
    /// Whether the stack map is described.
    pub debug_stackmap: bool,
    /// Whether class files are written in module specific subdirectories.
    pub multi_module_mode: std::cell::Cell<bool>,
    /// Dump the class modifiers (-XDdumpmodifiers=c).
    pub dump_class_modifiers: bool,
    /// Dump the field modifiers (-XDdumpmodifiers=f).
    pub dump_field_modifiers: bool,
    /// Dump the inner class modifiers (-XDdumpmodifiers=i).
    pub dump_inner_class_modifiers: bool,
    /// Dump the method modifiers (-XDdumpmodifiers=m).
    pub dump_method_modifiers: bool,
    /// The extra attribute writers.
    pub extra_attribute_hooks: RefCell<Vec<ExtraAttributeHook>>,
}

impl ClassWriter {
    /// Create the class file writer.
    ///
    /// # Arguments
    ///   - log: the diagnostic log;
    ///   - names: the name table;
    ///   - syms: the symbol table;
    ///   - types: the type relations;
    ///   - check: the check service;
    ///   - options: the compiler options;
    ///   - target: the code generation target;
    ///   - source: the source language level;
    ///   - preview: the preview feature helper;
    ///   - pool_writer: the constant pool writer;
    ///   - method_codes: the code objects of the generated methods.
    ///
    /// # Returns
    ///   - the initialized class writer.
    ///
    /// # Panics
    ///   - none.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        log: Rc<Log>,
        names: Rc<Names>,
        syms: Rc<crate::code::symtab::Symtab>,
        types: Rc<Types>,
        check: Rc<Check>,
        options: Rc<Options>,
        target: Target,
        source: Option<Source>,
        preview: Rc<Preview>,
        pool_writer: Rc<RefCell<PoolWriter>>,
        method_codes: Rc<RefCell<HashMap<SymbolId, Rc<RefCell<Code>>>>>,
    ) -> ClassWriter {
        let verbose = options.is_set("verbose");
        // Java: options.isUnset(G_CUSTOM) || options.isSet(G_CUSTOM, "source")
        // (ClassWriter.java:183-184). Options keys are the Option primary names
        // ("-g:"), not the enum constant names; the earlier string used the
        // Rust variant name and never matched put_option("-g:", ...).
        let emit_source_file = options
            .is_unset(crate::main::option::Option::G_CUSTOM.primary_name())
            || options.is_set_choice(crate::main::option::Option::G_CUSTOM, "source");
        let gen_crt = options.is_set("Xjcov");
        let debug_stackmap = options.is_set("debug.stackmap");
        let modifier_flags = options.get("debug.dumpmodifiers");
        let (mut dump_class, mut dump_field, mut dump_inner, mut dump_method) =
            (false, false, false, false);
        if let Some(flags) = &modifier_flags {
            dump_class = flags.contains('c');
            dump_field = flags.contains('f');
            dump_inner = flags.contains('i');
            dump_method = flags.contains('m');
        }
        ClassWriter {
            log,
            names,
            syms,
            types,
            check,
            options,
            target,
            source,
            preview,
            databuf: RefCell::new(ByteBuffer::with_initial_size(0x0fff0)),
            poolbuf: RefCell::new(ByteBuffer::with_initial_size(0x1fff0)),
            pool_writer,
            method_codes,
            verbose,
            emit_source_file,
            gen_crt,
            debug_stackmap,
            multi_module_mode: std::cell::Cell::new(false),
            dump_class_modifiers: dump_class,
            dump_field_modifiers: dump_field,
            dump_inner_class_modifiers: dump_inner,
            dump_method_modifiers: dump_method,
            extra_attribute_hooks: RefCell::new(Vec::new()),
        }
    }

    /// Add an extra attribute writer.
    pub fn add_extra_attributes(
        &self,
        extra: ExtraAttributeHook,
    ) {
        self.extra_attribute_hooks.borrow_mut().push(extra);
    }

    /// Return the flag names of the given flags as a space separated list.
    ///
    /// # Arguments
    ///   - flags: the flags to print.
    ///
    /// # Returns
    ///   - the concatenated flag names.
    ///
    /// # Panics
    ///   - none.
    pub fn flag_names(flags: i64) -> String {
        let mut result = String::new();
        let mut f = flags & STANDARD_FLAGS;
        let mut i = 0;
        while f != 0 {
            if (f & 1) != 0 {
                result.push(' ');
                result.push_str(FLAG_NAMES[i]);
            }
            f >>= 1;
            i += 1;
        }
        result
    }

    /// Write a character into the given byte buffer (no growth).
    pub fn put_char(buf: &mut ByteBuffer, op: i32, x: i32) {
        buf.elems[op as usize] = ((x >> 8) & 0xFF) as u8;
        buf.elems[(op + 1) as usize] = (x & 0xFF) as u8;
    }

    /// Write an integer into the given byte buffer (no growth).
    pub fn put_int(buf: &mut ByteBuffer, adr: i32, x: i32) {
        buf.elems[adr as usize] = ((x >> 24) & 0xFF) as u8;
        buf.elems[(adr + 1) as usize] = ((x >> 16) & 0xFF) as u8;
        buf.elems[(adr + 2) as usize] = ((x >> 8) & 0xFF) as u8;
        buf.elems[(adr + 3) as usize] = (x & 0xFF) as u8;
    }

    /// Write the header of an attribute and return the position past its length
    /// index.
    pub fn write_attr(&self, ctx: &mut CompilerContext, attr_name: Name) -> i32 {
        let index = self.pool_writer.borrow_mut().put_name(attr_name);
        self.databuf.borrow_mut().append_char(index);
        self.databuf.borrow_mut().append_int(0);
        let _ = ctx;
        self.databuf.borrow().length
    }

    /// Fill in the length of an attribute.
    pub fn end_attr(&self, index: i32) {
        let length = self.databuf.borrow().length - index;
        ClassWriter::put_int(&mut *self.databuf.borrow_mut(), index - 4, length);
    }

    /// Leave space for an attribute count and return its index.
    pub fn begin_attrs(&self) -> i32 {
        self.databuf.borrow_mut().append_char(0);
        self.databuf.borrow().length
    }

    /// Fill in an attribute count.
    pub fn end_attrs(&self, index: i32, count: i32) {
        ClassWriter::put_char(&mut *self.databuf.borrow_mut(), index - 2, count);
    }

    /// Write the EnclosingMethod attribute if needed and return the number of
    /// attributes written.
    pub fn write_enclosing_method_attribute(
        &self,
        ctx: &mut CompilerContext,
        c: SymbolId,
    ) -> i32 {
        let name = self.names.EnclosingMethod.clone();
        self.write_enclosing_method_attribute_named(ctx, name, c)
    }

    /// Write the EnclosingMethod attribute with a specified name.
    pub fn write_enclosing_method_attribute_named(
        &self,
        ctx: &mut CompilerContext,
        attribute_name: Name,
        c: SymbolId,
    ) -> i32 {
        let (owner, c_name) = (ctx.sym(c).owner, ctx.sym(c).name.clone());
        let owner_kind = owner.map(|owner| ctx.sym(owner).kind);
        if owner_kind != Some(crate::code::kinds::Kind::MTH)
            && c_name != self.names.empty
        {
            return 0;
        }
        let alen_idx = self.write_attr(ctx, attribute_name);
        let encl_class = crate::code::symbol::symbol_encl_class(ctx, owner.expect("owner"));
        let encl_class = encl_class.expect("enclosing class");
        let encl_method = {
            let owner = owner.expect("owner");
            let owner_ty = ctx.sym(owner).ty;
            if owner_ty.is_none() || ctx.sym(owner).kind != crate::code::kinds::Kind::MTH {
                None
            } else {
                original_enclosing_method(ctx, owner)
            }
        };
        let syms = Rc::clone(&self.syms);
        let encl_class_index = {
            let mut pool = self.pool_writer.borrow_mut();
            pool.put_class(ctx, &*syms, ctx.sym(encl_class).ty.expect("class type"))
        };
        self.databuf.borrow_mut().append_char(encl_class_index);
        let method_index = match encl_method {
            None => 0,
            Some(encl_method) => {
                let mut pool = self.pool_writer.borrow_mut();
                pool.put_name_and_type(ctx, &*syms, encl_method)
            }
        };
        self.databuf.borrow_mut().append_char(method_index);
        self.end_attr(alen_idx);
        1
    }

    /// Write the flag attributes and return the number written.
    pub fn write_flag_attrs(&self, ctx: &mut CompilerContext, flags_value: i64) -> i32 {
        let mut acount = 0;
        if (flags_value & flags::DEPRECATED as i64) != 0 {
            let name = self.names.Deprecated.clone();
            let alen_idx = self.write_attr(ctx, name);
            self.end_attr(alen_idx);
            acount += 1;
        }
        acount
    }

    /// Write the member attributes and return the number written.
    pub fn write_member_attrs(
        &self,
        ctx: &mut CompilerContext,
        sym: SymbolId,
        is_record_component: bool,
    ) -> i32 {
        let mut acount = 0;
        let sym_flags = ctx.sym(sym).flags();
        if !is_record_component {
            acount = self.write_flag_attrs(ctx, sym_flags);
        }
        let sym_ty = ctx.sym(sym).ty.expect("member type");
        let syms = Rc::clone(&self.syms);
        // Java: ClassWriter.java:363-370 compares sym.type with
        // sym.erasure(types) (the symbol's erasure_field) and checks the
        // thrown types of sym.type, not of the erasure.
        let erased = self.types.symbol_erasure(ctx, &*syms, sym);
        let same = self
            .types
            .is_same_type(ctx, &*syms, sym_ty, erased);
        let thrown = ctx.typ(sym_ty).get_thrown_types(ctx);
        let has_type_var = {
            let pool = self.pool_writer.borrow();
            pool.signature_gen.has_type_var(ctx, thrown.clone())
        };
        let is_synthetic = (sym_flags & flags::SYNTHETIC as i64) != 0;
        let is_bridge = (sym_flags & flags::BRIDGE) != 0;
        if (sym_flags & (flags::SYNTHETIC as i64 | flags::BRIDGE))
            != flags::SYNTHETIC as i64
            && (sym_flags & flags::ANONCONSTR as i64) == 0
            && (!same || has_type_var)
        {
            let name = self.names.Signature.clone();
            let alen_idx = self.write_attr(ctx, name);
            let index = self
                .pool_writer
                .borrow_mut()
                .put_signature(ctx, &*syms, sym);
            self.databuf.borrow_mut().append_char(index);
            self.end_attr(alen_idx);
            acount += 1;
        }
        let _ = (is_synthetic, is_bridge);
        let raw = ctx.sym(sym).get_raw_attributes();
        acount += self.write_java_annotations(ctx, raw);
        let raw_type = ctx.sym(sym).get_raw_type_attributes();
        acount += self.write_type_annotations(ctx, raw_type, false);
        acount
    }
}

/// Return the original enclosing method of a symbol.
/// Java: ((MethodSymbol)c.owner).originalEnclosingMethod() (ClassWriter.java:335);
/// a lambda synthetic method carries the original enclosing method
/// (LambdaToMethod.java:1944-1951, Symbol.java:2295-2298).
fn original_enclosing_method(ctx: &CompilerContext, owner: SymbolId) -> Option<SymbolId> {
    if ctx.sym(owner).kind == crate::code::kinds::Kind::MTH {
        // Java: MethodSymbol.originalEnclosingMethod() returns the override
        // installed by lambda translation, or the symbol itself.
        Some(crate::code::symbol::original_enclosing_method(ctx, owner))
    } else {
        None
    }
}

impl ClassWriter {
    /// Write the MethodParameters attribute and return 1 when written.
    pub fn write_method_parameters_attr(
        &self,
        ctx: &mut CompilerContext,
        m: SymbolId,
    ) -> i32 {
        let syms = Rc::clone(&self.syms);
        let ext = crate::code::symbol::external_type(
            ctx,
            &self.types,
            &*syms,
            &self.names,
            m,
        );
        let argtypes = match &ctx.typ(ext).kind {
            TypeKindData::MethodType { argtypes, .. } => argtypes.clone(),
            _ => List::nil(),
        };
        let allparams = argtypes.length();
        let (params, extra_params, captured_locals) = method_symbol_parts(ctx, m);
        if allparams == 0 {
            return 0;
        }
        let attr_name = self.names.MethodParameters.clone();
        let attr_index = self.write_attr(ctx, attr_name);
        self.databuf.borrow_mut().append_byte(allparams);
        let m_flags = ctx.sym(m).flags();
        for s in &extra_params {
            let s_flags = ctx.sym(*s).flags();
            let s_flags = (s_flags
                & (flags::FINAL as i64 | flags::SYNTHETIC as i64 | flags::MANDATED as i64))
                | (m_flags & flags::SYNTHETIC as i64);
            let name = ctx.sym(*s).name.clone();
            let index = self.pool_writer.borrow_mut().put_name(name);
            self.databuf.borrow_mut().append_char(index);
            self.databuf.borrow_mut().append_char(s_flags as i32);
        }
        for s in &params {
            let s_flags = ctx.sym(*s).flags();
            let s_flags = (s_flags
                & (flags::FINAL as i64 | flags::SYNTHETIC as i64 | flags::MANDATED as i64))
                | (m_flags & flags::SYNTHETIC as i64);
            let name = ctx.sym(*s).name.clone();
            let index = self.pool_writer.borrow_mut().put_name(name);
            self.databuf.borrow_mut().append_char(index);
            self.databuf.borrow_mut().append_char(s_flags as i32);
        }
        for s in &captured_locals {
            let s_flags = ctx.sym(*s).flags();
            let s_flags = (s_flags
                & (flags::FINAL as i64 | flags::SYNTHETIC as i64 | flags::MANDATED as i64))
                | (m_flags & flags::SYNTHETIC as i64);
            let name = ctx.sym(*s).name.clone();
            let index = self.pool_writer.borrow_mut().put_name(name);
            self.databuf.borrow_mut().append_char(index);
            self.databuf.borrow_mut().append_char(s_flags as i32);
        }
        self.end_attr(attr_index);
        1
    }

    /// Write the parameter annotations of a list of parameters.
    fn write_param_annotations(
        &self,
        ctx: &mut CompilerContext,
        params: &[SymbolId],
        retention: RetentionPolicy,
    ) {
        self.databuf.borrow_mut().append_byte(params.len() as i32);
        for s in params {
            let mut buf: Vec<AttributeId> = Vec::new();
            let raw = ctx.sym(*s).get_raw_attributes();
            let mut current = raw;
            for a in &current {
            if self.attribute_retention(ctx, a) == retention {
                buf.push(a);
            }
            }
            self.databuf.borrow_mut().append_char(buf.len() as i32);
            for a in buf {
                self.write_compound_attribute(ctx, a);
            }
        }
    }

    /// Write method parameter annotations.
    pub fn write_parameter_attrs(
        &self,
        ctx: &mut CompilerContext,
        params: &[SymbolId],
    ) -> i32 {
        let mut has_visible = false;
        let mut has_invisible = false;
        for s in params {
            let raw = ctx.sym(*s).get_raw_attributes();
            let mut current = raw;
            for a in &current {
            match self.attribute_retention(ctx, a) {
                RetentionPolicy::SOURCE => {}
                RetentionPolicy::CLASS => has_invisible = true,
                RetentionPolicy::RUNTIME => has_visible = true,
            }
            }
        }
        let mut attr_count = 0;
        if has_visible {
            let name = self.names.RuntimeVisibleParameterAnnotations.clone();
            let attr_index = self.write_attr(ctx, name);
            self.write_param_annotations(ctx, params, RetentionPolicy::RUNTIME);
            self.end_attr(attr_index);
            attr_count += 1;
        }
        if has_invisible {
            let name = self.names.RuntimeInvisibleParameterAnnotations.clone();
            let attr_index = self.write_attr(ctx, name);
            self.write_param_annotations(ctx, params, RetentionPolicy::CLASS);
            self.end_attr(attr_index);
            attr_count += 1;
        }
        attr_count
    }

    /// Return the retention policy of an attribute.
    fn attribute_retention(&self, ctx: &mut CompilerContext, attr: AttributeId) -> RetentionPolicy {
        let annotation_ty = match ctx.attr(attr).ty {
            Some(ty) => ty,
            None => return RetentionPolicy::CLASS,
        };
        let tsym = match ctx.typ(annotation_ty).tsym {
            Some(tsym) => tsym,
            None => return RetentionPolicy::CLASS,
        };
        let syms = Rc::clone(&self.syms);
        self.types.get_retention(ctx, &*syms, tsym)
    }

    /// Write the Java annotation attributes and return the number written.
    pub fn write_java_annotations(
        &self,
        ctx: &mut CompilerContext,
        attrs: List<AttributeId>,
    ) -> i32 {
        if attrs.is_empty() {
            return 0;
        }
        let mut visibles: Vec<AttributeId> = Vec::new();
        let mut invisibles: Vec<AttributeId> = Vec::new();
        let mut current = attrs;
        for a in &current {
        match self.attribute_retention(ctx, a) {
            RetentionPolicy::SOURCE => {}
            RetentionPolicy::CLASS => invisibles.push(a),
            RetentionPolicy::RUNTIME => visibles.push(a),
        }
        }
        let mut attr_count = 0;
        if !visibles.is_empty() {
            let name = self.names.RuntimeVisibleAnnotations.clone();
            let attr_index = self.write_attr(ctx, name);
            self.databuf.borrow_mut().append_char(visibles.len() as i32);
            for a in visibles {
                self.write_compound_attribute(ctx, a);
            }
            self.end_attr(attr_index);
            attr_count += 1;
        }
        if !invisibles.is_empty() {
            let name = self.names.RuntimeInvisibleAnnotations.clone();
            let attr_index = self.write_attr(ctx, name);
            self.databuf.borrow_mut().append_char(invisibles.len() as i32);
            for a in invisibles {
                self.write_compound_attribute(ctx, a);
            }
            self.end_attr(attr_index);
            attr_count += 1;
        }
        attr_count
    }

    /// Write the type annotation attributes and return the number written.
    pub fn write_type_annotations(
        &self,
        ctx: &mut CompilerContext,
        type_annos: List<AttributeId>,
        in_code: bool,
    ) -> i32 {
        if type_annos.is_empty() {
            return 0;
        }
        let mut visibles: Vec<AttributeId> = Vec::new();
        let mut invisibles: Vec<AttributeId> = Vec::new();
        let mut current = type_annos;
        for tc in &current {
        let has_unknown = ctx.attr(tc).has_unknown_position();
        if has_unknown {
            let mut attribute = ctx.attr(tc).clone();
            let fixed = attribute.try_fix_position(ctx);
            if !fixed {
                // This happens for nested types like @A Outer. @B Inner.
                eprintln!(
                    "ClassWriter: Position UNKNOWN in type annotation: {:?}",
                    tc
                );
                continue;
            }
            *ctx.attr_mut(tc) = attribute;
        }
        let position = match ctx.attr(tc).get_position(ctx) {
            Some(position) => position,
            None => continue,
        };
        if position.type_.is_local() != in_code {
            continue;
        }
        if !position.emit_to_classfile() {
            continue;
        }
        match self.attribute_retention(ctx, tc) {
            RetentionPolicy::SOURCE => {}
            RetentionPolicy::CLASS => invisibles.push(tc),
            RetentionPolicy::RUNTIME => visibles.push(tc),
        }
        }
        let mut attr_count = 0;
        if !visibles.is_empty() {
            let name = self.names.RuntimeVisibleTypeAnnotations.clone();
            let attr_index = self.write_attr(ctx, name);
            self.databuf.borrow_mut().append_char(visibles.len() as i32);
            for p in visibles {
                self.write_type_annotation(ctx, p);
            }
            self.end_attr(attr_index);
            attr_count += 1;
        }
        if !invisibles.is_empty() {
            let name = self.names.RuntimeInvisibleTypeAnnotations.clone();
            let attr_index = self.write_attr(ctx, name);
            self.databuf.borrow_mut().append_char(invisibles.len() as i32);
            for p in invisibles {
                self.write_type_annotation(ctx, p);
            }
            self.end_attr(attr_index);
            attr_count += 1;
        }
        attr_count
    }

    /// Write one attribute value including its leading marker character.
    pub fn write_attribute(&self, ctx: &mut CompilerContext, attr: AttributeId) {
        let kind = ctx.attr(attr).kind.clone();
        match kind {
            AttributeKind::Constant { value } => {
                let ty = ctx.attr(attr).ty.expect("constant attribute type");
                if ctx.typ(ty).has_tag(crate::code::type_tag::TypeTag::CLASS) {
                    let s = match &value {
                        crate::util::constants::ConstantValue::String(s) => s.clone(),
                        _ => panic!("AssertionError: string constant expected"),
                    };
                    self.databuf.borrow_mut().append_byte('s' as i32);
                    let name = self.names.from_string(&s);
                    let index = self.pool_writer.borrow_mut().put_name(name);
                    self.databuf.borrow_mut().append_char(index);
                } else {
                    let tag = ctx.typ(ty).get_tag();
                    let marker = match tag {
                        crate::code::type_tag::TypeTag::BYTE => 'B',
                        crate::code::type_tag::TypeTag::CHAR => 'C',
                        crate::code::type_tag::TypeTag::SHORT => 'S',
                        crate::code::type_tag::TypeTag::INT => 'I',
                        crate::code::type_tag::TypeTag::LONG => 'J',
                        crate::code::type_tag::TypeTag::FLOAT => 'F',
                        crate::code::type_tag::TypeTag::DOUBLE => 'D',
                        crate::code::type_tag::TypeTag::BOOLEAN => 'Z',
                        other => panic!("AssertionError: unsupported constant tag {:?}", other),
                    };
                    self.databuf.borrow_mut().append_byte(marker as i32);
                    let syms = Rc::clone(&self.syms);
                    let index = self
                        .pool_writer
                        .borrow_mut()
                        .put_constant_value(ctx, &*syms, &value);
                    self.databuf.borrow_mut().append_char(index);
                }
            }
            AttributeKind::Enum { value } => {
                let value = value.expect("enum attribute value");
                self.databuf.borrow_mut().append_byte('e' as i32);
                let syms = Rc::clone(&self.syms);
                let ty = ctx.sym(value).ty.expect("enum symbol type");
                let index = {
                    let mut pool = self.pool_writer.borrow_mut();
                    pool.put_descriptor(ctx, &*syms, ty)
                };
                self.databuf.borrow_mut().append_char(index);
                let name = ctx.sym(value).name.clone();
                let index = self.pool_writer.borrow_mut().put_name(name);
                self.databuf.borrow_mut().append_char(index);
            }
            AttributeKind::Class { class_type } => {
                let class_type = class_type.expect("class attribute type");
                self.databuf.borrow_mut().append_byte('c' as i32);
                let syms = Rc::clone(&self.syms);
                let index = {
                    let mut pool = self.pool_writer.borrow_mut();
                    pool.put_descriptor(ctx, &*syms, class_type)
                };
                self.databuf.borrow_mut().append_char(index);
            }
            AttributeKind::Compound { .. } | AttributeKind::TypeCompound { .. } => {
                self.databuf.borrow_mut().append_byte('@' as i32);
                self.write_compound_attribute(ctx, attr);
            }
            AttributeKind::Error => {
                panic!("AssertionError: error attribute");
            }
            AttributeKind::Array { values } => {
                self.databuf.borrow_mut().append_byte('[' as i32);
                self.databuf.borrow_mut().append_char(values.len() as i32);
                for value in values {
                    self.write_attribute(ctx, value);
                }
            }
            AttributeKind::UnresolvedClass { .. } => {
                panic!("AssertionError: unresolved class attribute");
            }
        }
    }

    /// Write a compound attribute excluding the '@' marker.
    pub fn write_compound_attribute(&self, ctx: &mut CompilerContext, c: AttributeId) {
        let ty = ctx.attr(c).ty.expect("compound attribute type");
        let syms = Rc::clone(&self.syms);
        let index = {
            let mut pool = self.pool_writer.borrow_mut();
            pool.put_descriptor(ctx, &*syms, ty)
        };
        self.databuf.borrow_mut().append_char(index);
        let values = match &ctx.attr(c).kind {
            AttributeKind::Compound { values, .. } | AttributeKind::TypeCompound { values, .. } => {
                values.clone()
            }
            _ => panic!("AssertionError: compound attribute expected"),
        };
        self.databuf.borrow_mut().append_char(values.length());
        let mut current = values;
        for pair in &current {
        let (symbol, value) = (pair.fst, pair.snd);
        let name = ctx.sym(symbol).name.clone();
        let index = self.pool_writer.borrow_mut().put_name(name);
        self.databuf.borrow_mut().append_char(index);
        self.write_attribute(ctx, value);
        }
    }

    /// Write a type annotation attribute.
    pub fn write_type_annotation(&self, ctx: &mut CompilerContext, c: AttributeId) {
        let position = ctx
            .attr(c)
            .get_position(ctx)
            .expect("type annotation position");
        self.write_position(position);
        self.write_compound_attribute(ctx, c);
    }

    /// Write a type annotation position.
    pub fn write_position(&self, p: TypeAnnotationPosition) {
        self.databuf.borrow_mut().append_byte(p.type_.target_type_value());
        use TargetType::*;
        match p.type_ {
            INSTANCEOF | NEW | CONSTRUCTOR_REFERENCE | METHOD_REFERENCE => {
                self.databuf.borrow_mut().append_char(p.offset);
            }
            LOCAL_VARIABLE | RESOURCE_VARIABLE => {
                let offsets = p.lvar_offset.clone().expect("lvar offsets");
                let lengths = p.lvar_length.clone().expect("lvar lengths");
                let indices = p.lvar_index.clone().expect("lvar indices");
                self.databuf.borrow_mut().append_char(offsets.len() as i32);
                for i in 0..offsets.len() {
                    self.databuf.borrow_mut().append_char(offsets[i]);
                    self.databuf.borrow_mut().append_char(lengths[i]);
                    self.databuf.borrow_mut().append_char(indices[i]);
                }
            }
            EXCEPTION_PARAMETER => {
                self.databuf.borrow_mut().append_char(p.get_exception_index());
            }
            METHOD_RECEIVER => {}
            CLASS_TYPE_PARAMETER | METHOD_TYPE_PARAMETER => {
                self.databuf.borrow_mut().append_byte(p.parameter_index);
            }
            CLASS_TYPE_PARAMETER_BOUND | METHOD_TYPE_PARAMETER_BOUND => {
                self.databuf.borrow_mut().append_byte(p.parameter_index);
                self.databuf.borrow_mut().append_byte(p.bound_index);
            }
            CLASS_EXTENDS => {
                self.databuf.borrow_mut().append_char(p.type_index);
            }
            THROWS => {
                self.databuf.borrow_mut().append_char(p.type_index);
            }
            METHOD_FORMAL_PARAMETER => {
                self.databuf.borrow_mut().append_byte(p.parameter_index);
            }
            CAST
            | CONSTRUCTOR_INVOCATION_TYPE_ARGUMENT
            | METHOD_INVOCATION_TYPE_ARGUMENT
            | CONSTRUCTOR_REFERENCE_TYPE_ARGUMENT
            | METHOD_REFERENCE_TYPE_ARGUMENT => {
                self.databuf.borrow_mut().append_char(p.offset);
                self.databuf.borrow_mut().append_byte(p.type_index);
            }
            METHOD_RETURN | FIELD => {}
            UNKNOWN => {
                panic!("AssertionError: jvm.ClassWriter: UNKNOWN target type should never occur!");
            }
        }
        let loc = TypeAnnotationPosition::get_binary_from_type_path(p.location.clone());
        // Java: appendByte(loc.length() / 2) — type_path.path_length counts
        // entries, while getBinaryFromTypePath returns two bytes per entry.
        self.databuf.borrow_mut().append_byte(loc.length() / 2);
        let mut current = loc;
        while current.non_empty() {
            let code = *current.head();
            self.databuf.borrow_mut().append_byte(code);
            current = current.tail();
        }
    }
}

impl ClassWriter {
    /// Write the Module attribute if needed and return the number written.
    pub fn write_module_attribute(&self, ctx: &mut CompilerContext, c: SymbolId) -> i32 {
        let m = ctx.sym(c).owner.expect("module owner");
        let (
            version,
            requires,
            exports,
            opens,
            uses,
            provides,
            module_flags,
            unnamed_package,
        ) = match &ctx.sym(m).kind_data {
            SymbolKind::ModuleSymbol {
                version,
                requires,
                exports,
                opens,
                uses,
                provides,
                flags: module_flags,
                unnamed_package,
                ..
            } => (
                version.clone(),
                requires.clone(),
                exports.clone(),
                opens.clone(),
                uses.clone(),
                provides.clone(),
                module_flags.clone(),
                *unnamed_package,
            ),
            _ => panic!("AssertionError: module symbol expected"),
        };
        let attr_name = self.names.Module.clone();
        let alen_idx = self.write_attr(ctx, attr_name);
        let module_index = self
            .pool_writer
            .borrow_mut()
            .put_module(ctx, &self.syms, m);
        self.databuf.borrow_mut().append_char(module_index);
        self.databuf.borrow_mut().append_char(crate::code::symbol::ModuleFlags::value_of(&module_flags));
        let version_index = match version {
            None => 0,
            Some(version) => self.pool_writer.borrow_mut().put_name(version),
        };
        self.databuf.borrow_mut().append_char(version_index);
        let mut requires_kept: Vec<SymbolId> = Vec::new();
        let requires_all = requires.clone();
        let mut current = requires_all.clone();
        for id in &current {
        if let Directive::Requires { module, flags: rflags } = ctx.directive(id)
            && !rflags.contains(&RequiresFlag::EXTRA)
        {
            requires_kept.push(module.expect("requires module"));
        }
        }
        self.databuf.borrow_mut().append_char(requires_kept.len() as i32);
        for r in requires_kept {
            let index = self
                .pool_writer
                .borrow_mut()
                .put_module(ctx, &self.syms, r);
            self.databuf.borrow_mut().append_char(index);
            // The requires flags are read from the directive.
            let mut current = requires_all.clone();
            let mut flags_of = Vec::new();
            for id in &current {
            if let Directive::Requires { module: Some(module), flags: rflags } = ctx.directive(id)
                && *module == r
            {
                flags_of = rflags.clone();
            }
            }
            self.databuf.borrow_mut().append_char(RequiresFlag::value_of(&flags_of));
            let version_index = match &ctx.sym(r).kind_data {
                SymbolKind::ModuleSymbol { version: Some(version), .. } => {
                    self.pool_writer.borrow_mut().put_name(version.clone())
                }
                _ => 0,
            };
            self.databuf.borrow_mut().append_char(version_index);
        }
        // exports
        let mut exports_list: Vec<(SymbolId, Vec<ExportsFlag>, Option<List<SymbolId>>)> = Vec::new();
        let mut current = exports;
        for id in &current {
        if let Directive::Exports { packge, modules, flags: eflags } = ctx.directive(id) {
            exports_list.push((
                packge.expect("exports package"),
                eflags.clone(),
                modules.clone(),
            ));
        }
        }
        self.databuf.borrow_mut().append_char(exports_list.len() as i32);
        for (packge, eflags, modules) in exports_list {
            let index = self
                .pool_writer
                .borrow_mut()
                .put_package(ctx, &self.syms, packge);
            self.databuf.borrow_mut().append_char(index);
            self.databuf.borrow_mut().append_char(ExportsFlag::value_of(&eflags));
            match modules {
                None => {
                    self.databuf.borrow_mut().append_char(0);
                }
                Some(modules) => {
                    self.databuf.borrow_mut().append_char(modules.length());
                    let mut l = modules;
                    for msym in &l {
                    let index = self
                        .pool_writer
                        .borrow_mut()
                        .put_module(ctx, &self.syms, msym);
                    self.databuf.borrow_mut().append_char(index);
                    }
                }
            }
        }
        // opens
        let mut opens_list: Vec<(SymbolId, Vec<OpensFlag>, Option<List<SymbolId>>)> = Vec::new();
        let mut current = opens;
        for id in &current {
        if let Directive::Opens { packge, modules, flags: oflags } = ctx.directive(id) {
            opens_list.push((
                packge.expect("opens package"),
                oflags.clone(),
                modules.clone(),
            ));
        }
        }
        self.databuf.borrow_mut().append_char(opens_list.len() as i32);
        for (packge, oflags, modules) in opens_list {
            let index = self
                .pool_writer
                .borrow_mut()
                .put_package(ctx, &self.syms, packge);
            self.databuf.borrow_mut().append_char(index);
            self.databuf.borrow_mut().append_char(OpensFlag::value_of(&oflags));
            match modules {
                None => {
                    self.databuf.borrow_mut().append_char(0);
                }
                Some(modules) => {
                    self.databuf.borrow_mut().append_char(modules.length());
                    let mut l = modules;
                    for msym in &l {
                    let index = self
                        .pool_writer
                        .borrow_mut()
                        .put_module(ctx, &self.syms, msym);
                    self.databuf.borrow_mut().append_char(index);
                    }
                }
            }
        }
        // uses
        let mut uses_list: Vec<SymbolId> = Vec::new();
        let mut current = uses;
        for id in &current {
        if let Directive::Uses { service: Some(service) } = ctx.directive(id) {
            uses_list.push(*service);
        }
        }
        self.databuf.borrow_mut().append_char(uses_list.len() as i32);
        for service in uses_list {
            let index = self
                .pool_writer
                .borrow_mut()
                .put_class_symbol(ctx, &self.syms, service);
            self.databuf.borrow_mut().append_char(index);
        }
        // provides (merged by service)
        let mut merged: Vec<(SymbolId, Vec<SymbolId>)> = Vec::new();
        let mut current = provides;
        for id in &current {
        if let Directive::Provides { service: Some(service), impls } = ctx.directive(id) {
            match merged.iter_mut().find(|(s, _)| *s == *service) {
                Some((_, list)) => {
                    let mut l = impls.clone();
                    while l.non_empty() {
                        list.push(*l.head());
                        l = l.tail();
                    }
                }
                None => {
                    let mut list = Vec::new();
                    let mut l = impls.clone();
                    while l.non_empty() {
                        list.push(*l.head());
                        l = l.tail();
                    }
                    merged.push((*service, list));
                }
            }
        }
        }
        self.databuf.borrow_mut().append_char(merged.len() as i32);
        for (service, impls) in merged {
            let index = self
                .pool_writer
                .borrow_mut()
                .put_class_symbol(ctx, &self.syms, service);
            self.databuf.borrow_mut().append_char(index);
            self.databuf.borrow_mut().append_char(impls.len() as i32);
            for impl_ in impls {
                let index = self
                    .pool_writer
                    .borrow_mut()
                    .put_class_symbol(ctx, &self.syms, impl_);
                self.databuf.borrow_mut().append_char(index);
            }
        }
        let _ = unnamed_package;
        self.end_attr(alen_idx);
        1
    }

    /// Write the InnerClasses attribute.
    pub fn write_inner_classes(&self, ctx: &mut CompilerContext) {
        let attr_name = self.names.InnerClasses.clone();
        let alen_idx = self.write_attr(ctx, attr_name);
        let inner_classes = self.pool_writer.borrow().inner_classes.clone();
        self.databuf.borrow_mut().append_char(inner_classes.len() as i32);
        for inner in inner_classes {
            let mut flags_value = adjust_flags(ctx.sym(inner).flags_field);
            if (flags_value & flags::INTERFACE) != 0 {
                flags_value |= flags::ABSTRACT;
            }
            flags_value &= !flags::STRICTFP;
            if self.dump_inner_class_modifiers {
                eprintln!("INNERCLASS  {}", ctx.sym(inner).name.to_java_string().to_string_lossy());
                eprintln!("---{}", ClassWriter::flag_names(flags_value as i64));
            }
            let index = self
                .pool_writer
                .borrow_mut()
                .put_class_symbol(ctx, &self.syms, inner);
            self.databuf.borrow_mut().append_char(index);
            let (owner, name) = (ctx.sym(inner).owner, ctx.sym(inner).name.clone());
            let owner_index = match owner {
                Some(owner)
                    if ctx.sym(owner).kind == crate::code::kinds::Kind::TYP
                        && !name.is_empty() =>
                {
                    self.pool_writer
                        .borrow_mut()
                        .put_class_symbol(ctx, &self.syms, owner)
                }
                _ => 0,
            };
            self.databuf.borrow_mut().append_char(owner_index);
            let name_index = if !name.is_empty() {
                self.pool_writer.borrow_mut().put_name(name)
            } else {
                0
            };
            self.databuf.borrow_mut().append_char(name_index);
            self.databuf.borrow_mut().append_char(flags_value);
        }
        self.end_attr(alen_idx);
    }

    /// Write the Record attribute and return 1.
    pub fn write_record_attribute(&self, ctx: &mut CompilerContext, csym: SymbolId) -> i32 {
        let attr_name = self.names.Record.clone();
        let alen_idx = self.write_attr(ctx, attr_name);
        let components = record_components(ctx, csym);
        self.databuf.borrow_mut().append_char(components.length());
        let mut current = components;
        for v in &current {
        let name = ctx.sym(v).name.clone();
        let name_index = self.pool_writer.borrow_mut().put_name(name);
        self.databuf.borrow_mut().append_char(name_index);
        let descriptor = self
            .pool_writer
            .borrow_mut()
            .put_descriptor_symbol(ctx, &self.syms, v);
        self.databuf.borrow_mut().append_char(descriptor);
        let acount_idx = self.begin_attrs();
        let mut acount = 0;
        acount += self.write_member_attrs(ctx, v, true);
        self.end_attrs(acount_idx, acount);
        }
        self.end_attr(alen_idx);
        1
    }

    /// Write the NestMembers attribute if needed and return the number written.
    pub fn write_nest_members_if_needed(
        &self,
        ctx: &mut CompilerContext,
        csym: SymbolId,
    ) -> i32 {
        let mut nested: Vec<SymbolId> = Vec::new();
        list_nested(ctx, csym, &mut nested);
        let mut nested_unique: Vec<SymbolId> = Vec::new();
        for n in nested {
            if !nested_unique.contains(&n) {
                nested_unique.push(n);
            }
        }
        let owner = ctx.sym(csym).owner.expect("class owner");
        if ctx.sym(owner).kind == crate::code::kinds::Kind::PCK
            && !nested_unique.is_empty()
        {
            let attr_name = self.names.NestMembers.clone();
            let alen_idx = self.write_attr(ctx, attr_name);
            self.databuf.borrow_mut().append_char(nested_unique.len() as i32);
            for s in nested_unique {
                let index = self
                    .pool_writer
                    .borrow_mut()
                    .put_class_symbol(ctx, &self.syms, s);
                self.databuf.borrow_mut().append_char(index);
            }
            self.end_attr(alen_idx);
            1
        } else {
            0
        }
    }

    /// Write the NestHost attribute if needed and return the number written.
    pub fn write_nest_host_if_needed(
        &self,
        ctx: &mut CompilerContext,
        csym: SymbolId,
    ) -> i32 {
        let owner = ctx.sym(csym).owner.expect("class owner");
        if ctx.sym(owner).kind != crate::code::kinds::Kind::PCK {
            let attr_name = self.names.NestHost.clone();
            let alen_idx = self.write_attr(ctx, attr_name);
            let outermost = crate::code::symbol::symbol_outermost_class(ctx, csym)
                .expect("outermost class");
            let index = self
                .pool_writer
                .borrow_mut()
                .put_class_symbol(ctx, &self.syms, outermost);
            self.databuf.borrow_mut().append_char(index);
            self.end_attr(alen_idx);
            1
        } else {
            0
        }
    }

    /// Write the PermittedSubclasses attribute if needed and return the number
    /// written.
    pub fn write_permitted_subclasses_if_needed(
        &self,
        ctx: &mut CompilerContext,
        csym: SymbolId,
    ) -> i32 {
        let permitted = match &ctx.sym(csym).kind_data {
            SymbolKind::ClassSymbol { permitted, .. } => permitted.clone(),
            _ => List::nil(),
        };
        if permitted.non_empty() {
            let attr_name = self.names.PermittedSubclasses.clone();
            let alen_idx = self.write_attr(ctx, attr_name);
            self.databuf.borrow_mut().append_char(permitted.length());
            let mut current = permitted;
            for c in &current {
            let index = self
                .pool_writer
                .borrow_mut()
                .put_class_symbol(ctx, &self.syms, c);
            self.databuf.borrow_mut().append_char(index);
            }
            self.end_attr(alen_idx);
            1
        } else {
            0
        }
    }

    /// Write the BootstrapMethods attribute.
    pub fn write_bootstrap_methods(&self, ctx: &mut CompilerContext) {
        let attr_name = self.names.BootstrapMethods.clone();
        let alen_idx = self.write_attr(ctx, attr_name);
        let bootstrap = self.pool_writer.borrow().bootstrap_methods.clone();
        self.databuf.borrow_mut().append_char(bootstrap.len() as i32);
        for (bsm_key, _) in bootstrap {
            let bsm = bsm_key.bsm.expect("bootstrap method handle");
            let syms = Rc::clone(&self.syms);
            let index = {
                let mut pool = self.pool_writer.borrow_mut();
                pool.put_constant(
                    ctx,
                    &*syms,
                    crate::jvm::pool_writer::PoolConstantRef::MethodHandle(bsm),
                )
            };
            self.databuf.borrow_mut().append_char(index);
            self.databuf.borrow_mut().append_char(bsm_key.static_args.len() as i32);
            for arg in bsm_key.static_args {
                let syms = Rc::clone(&self.syms);
                let index = {
                    let mut pool = self.pool_writer.borrow_mut();
                    pool.put_constant(
                        ctx,
                        &*syms,
                        loadable_to_pool_constant(&arg),
                    )
                };
                self.databuf.borrow_mut().append_char(index);
            }
        }
        self.end_attr(alen_idx);
    }

    /// Write an extra attribute via the registered hooks and return the count.
    pub fn write_extra_attributes(&self, ctx: &mut CompilerContext, sym: SymbolId) -> i32 {
        let mut count = 0;
        let hooks = std::mem::take(&mut *self.extra_attribute_hooks.borrow_mut());
        for mut hook in hooks {
            count += hook(ctx, sym);
        }
        count
    }
}

/// Return the (params, extra params, captured locals) of a method symbol.
fn method_symbol_parts(
    ctx: &CompilerContext,
    m: SymbolId,
) -> (Vec<SymbolId>, Vec<SymbolId>, Vec<SymbolId>) {
    match &ctx.sym(m).kind_data {
        SymbolKind::MethodSymbol {
            params,
            extra_params,
            captured_locals,
            ..
        } => (to_vec(params), to_vec(extra_params), to_vec(captured_locals)),
        _ => (Vec::new(), Vec::new(), Vec::new()),
    }
}

/// Convert a symbol list to a vector.
fn to_vec(list: &List<SymbolId>) -> Vec<SymbolId> {
    let mut result = Vec::new();
    let mut current = list.clone();
    while current.non_empty() {
        result.push(*current.head());
        current = current.tail();
    }
    result
}

/// Return the record components of a class symbol.
fn record_components(ctx: &CompilerContext, c: SymbolId) -> List<SymbolId> {
    match &ctx.sym(c).kind_data {
        SymbolKind::ClassSymbol { record_components, .. } => record_components.clone(),
        _ => List::nil(),
    }
}

/// Collect the nested class symbols of a symbol.
fn list_nested(ctx: &mut CompilerContext, sym: SymbolId, seen: &mut Vec<SymbolId>) {
    if ctx.sym(sym).kind != crate::code::kinds::Kind::TYP {
        return;
    }
    if let Some(owner) = ctx.sym(sym).owner
        && ctx.sym(owner).kind != crate::code::kinds::Kind::PCK
    {
        seen.push(sym);
    }
    if let Some(members) = ctx.sym(sym).members() {
        let scope_snapshot = ctx.scope(members).clone();
        let symbols = scope_snapshot.get_symbols(
            members,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
            ctx,
        );
        for s in symbols {
            list_nested(ctx, s, seen);
        }
    }
    let trans_local = match &ctx.sym(sym).kind_data {
        SymbolKind::ClassSymbol { trans_local, .. } => trans_local.clone(),
        _ => List::nil(),
    };
    let mut current = trans_local;
    for s in &current {
    list_nested(ctx, s, seen);
    }
}

/// Return the output class file flags of a set of flags.
fn adjust_flags(flags_value: i64) -> i32 {
    let mut result = flags_value as i32;
    if (flags_value & flags::BRIDGE) != 0 {
        result |= flags::ACC_BRIDGE;
    }
    if (flags_value & flags::VARARGS) != 0 {
        result |= flags::ACC_VARARGS;
    }
    if (flags_value & flags::DEFAULT) != 0 {
        result &= !flags::ABSTRACT;
    }
    result
}

impl ClassWriter {
    /// Write a field symbol, entering all references into the constant pool.
    pub fn write_field(&self, ctx: &mut CompilerContext, v: SymbolId) {
        let field_flags = adjust_flags(ctx.sym(v).flags());
        self.databuf.borrow_mut().append_char(field_flags);
        if self.dump_field_modifiers {
            eprintln!("FIELD  {}", ctx.sym(v).name.to_java_string().to_string_lossy());
            eprintln!("---{}", ClassWriter::flag_names(ctx.sym(v).flags()));
        }
        let name = ctx.sym(v).name.clone();
        let name_index = self.pool_writer.borrow_mut().put_name(name);
        self.databuf.borrow_mut().append_char(name_index);
        let descriptor = self
            .pool_writer
            .borrow_mut()
            .put_descriptor_symbol(ctx, &self.syms, v);
        self.databuf.borrow_mut().append_char(descriptor);
        let acount_idx = self.begin_attrs();
        let mut acount = 0;
        if let Some(const_value) = ctx.sym(v).get_const_value() {
            let attr_name = self.names.ConstantValue.clone();
            let alen_idx = self.write_attr(ctx, attr_name);
            let syms = Rc::clone(&self.syms);
            let index = {
                let mut pool = self.pool_writer.borrow_mut();
                pool.put_constant_value(ctx, &*syms, &const_value)
            };
            self.databuf.borrow_mut().append_char(index);
            self.end_attr(alen_idx);
            acount += 1;
        }
        acount += self.write_member_attrs(ctx, v, false);
        acount += self.write_extra_attributes(ctx, v);
        self.end_attrs(acount_idx, acount);
    }

    /// Write a method symbol, entering all references into the constant pool.
    pub fn write_method(&self, ctx: &mut CompilerContext, m: SymbolId) {
        let method_flags = adjust_flags(ctx.sym(m).flags());
        self.databuf.borrow_mut().append_char(method_flags);
        if self.dump_method_modifiers {
            eprintln!("METHOD  {}", ctx.sym(m).name.to_java_string().to_string_lossy());
            eprintln!("---{}", ClassWriter::flag_names(ctx.sym(m).flags()));
        }
        let name = ctx.sym(m).name.clone();
        let name_index = self.pool_writer.borrow_mut().put_name(name);
        self.databuf.borrow_mut().append_char(name_index);
        let descriptor = self
            .pool_writer
            .borrow_mut()
            .put_descriptor_symbol(ctx, &self.syms, m);
        self.databuf.borrow_mut().append_char(descriptor);
        let acount_idx = self.begin_attrs();
        let mut acount = 0;
        let code = self.method_codes.borrow_mut().remove(&m);
        if let Some(code) = code {
            let attr_name = self.names.Code.clone();
            let alen_idx = self.write_attr(ctx, attr_name);
            self.write_code(ctx, &code);
            self.end_attr(alen_idx);
            acount += 1;
        }
        let sym_ty = ctx.sym(m).ty.expect("method type");
        let syms = Rc::clone(&self.syms);
        let erased = self.types.erasure(ctx, &*syms, sym_ty);
        let thrown = ctx.typ(erased).get_thrown_types(ctx);
        if thrown.non_empty() {
            let attr_name = self.names.Exceptions.clone();
            let alen_idx = self.write_attr(ctx, attr_name);
            self.databuf.borrow_mut().append_char(thrown.length());
            let mut l = thrown;
            for ty in &l {
            let tsym = ctx.typ(ty).tsym.expect("exception type symbol");
            let index = self
                .pool_writer
                .borrow_mut()
                .put_class_symbol(ctx, &self.syms, tsym);
            self.databuf.borrow_mut().append_char(index);
            }
            self.end_attr(alen_idx);
            acount += 1;
        }
        let default_value = match &ctx.sym(m).kind_data {
            SymbolKind::MethodSymbol { default_value, .. } => *default_value,
            _ => None,
        };
        if let Some(default_value) = default_value {
            let attr_name = self.names.AnnotationDefault.clone();
            let alen_idx = self.write_attr(ctx, attr_name);
            self.write_attribute(ctx, default_value);
            self.end_attr(alen_idx);
            acount += 1;
        }
        let is_ctor = ctx.sym(m).is_constructor(&self.names);
        let record_flag = (ctx.sym(m).flags_field & flags::RECORD) != 0;
        let is_lambda = is_lambda_method(ctx, m);
        if self.target.has_method_parameters()
            && (self
                .options
                .is_set_option(crate::main::option::Option::PARAMETERS)
                || (is_ctor && record_flag))
            && !is_lambda
        {
            acount += self.write_method_parameters_attr(ctx, m);
        }
        acount += self.write_member_attrs(ctx, m, false);
        let params = method_symbol_parts(ctx, m).0;
        if !is_lambda {
            acount += self.write_parameter_attrs(ctx, &params);
        }
        acount += self.write_extra_attributes(ctx, m);
        self.end_attrs(acount_idx, acount);
    }

    /// Write the Code attribute of a method.
    pub fn write_code(&self, ctx: &mut CompilerContext, code: &Rc<RefCell<Code>>) {
        let (max_stack, max_locals, cp, code_bytes, catch_info, line_info, var_debug_info, var_buffer, var_buffer_size, stack_map_buffer_size, stack_map, stack_map_buffer, stack_map_table_buffer, meth) = {
            let code = code.borrow();
            (
                code.max_stack,
                code.max_locals,
                code.cp,
                code.code.clone(),
                code.catch_info.clone(),
                code.line_info.clone(),
                code.var_debug_info,
                code.var_buffer.clone(),
                code.var_buffer_size,
                code.stack_map_buffer_size,
                code.stack_map,
                code.stack_map_buffer.clone(),
                code.stack_map_table_buffer.clone(),
                code.meth,
            )
        };
        self.databuf.borrow_mut().append_char(max_stack);
        self.databuf.borrow_mut().append_char(max_locals);
        self.databuf.borrow_mut().append_int(cp);
        self.databuf.borrow_mut().append_bytes_from(&code_bytes, 0, cp);
        self.databuf.borrow_mut().append_char(catch_info.len() as i32);
        for entry in &catch_info {
            for value in entry.iter() {
                self.databuf.borrow_mut().append_char(*value as i32);
            }
        }
        let acount_idx = self.begin_attrs();
        let mut acount = 0;
        if !line_info.is_empty() {
            let attr_name = self.names.LineNumberTable.clone();
            let alen_idx = self.write_attr(ctx, attr_name);
            self.databuf.borrow_mut().append_char(line_info.length());
            let mut l = line_info.reverse();
            while l.non_empty() {
                let entry = l.head();
                self.databuf.borrow_mut().append_char(entry[0] as i32);
                self.databuf.borrow_mut().append_char(entry[1] as i32);
                l = l.tail();
            }
            self.end_attr(alen_idx);
            acount += 1;
        }
        if self.gen_crt {
            let crt_exists = code.borrow().crt.is_some();
            if crt_exists {
                let attr_name = self.names.CharacterRangeTable.clone();
                let alen_idx = self.write_attr(ctx, attr_name);
                let crt_idx = self.begin_attrs();
                let entries = {
                    let mut code_ref = code.borrow_mut();
                    let line_map = code_ref.line_map.clone();
                    let log = Rc::clone(&self.log);
                    match &mut code_ref.crt {
                        Some(crt) => {
                            let line_map = line_map.expect("line map");
                            crt.write_crt(ctx, &mut *self.databuf.borrow_mut(), &*line_map, &log)
                        }
                        None => 0,
                    }
                };
                self.end_attrs(crt_idx, entries);
                self.end_attr(alen_idx);
                acount += 1;
            }
        }
        if var_debug_info && var_buffer_size > 0 {
            let mut n_generic_vars = 0;
            let attr_name = self.names.LocalVariableTable.clone();
            let mut alen_idx = self.write_attr(ctx, attr_name);
            let lvt_size = {
                let code = code.borrow();
                code.get_lvt_size()
            };
            self.databuf.borrow_mut().append_char(lvt_size);
            for i in 0..var_buffer_size {
                let var = match var_buffer.get(i as usize).and_then(|v| v.clone()) {
                    Some(var) => var,
                    None => continue,
                };
                for r in &var.alive_ranges {
                    self.databuf.borrow_mut().append_char(r.start_pc);
                    self.databuf.borrow_mut().append_char(r.length);
                    let name = ctx.sym(var.sym).name.clone();
                    let name_index = self.pool_writer.borrow_mut().put_name(name);
                    self.databuf.borrow_mut().append_char(name_index);
                    let descriptor = self.pool_writer.borrow_mut().put_descriptor_symbol(
                        ctx,
                        &self.syms,
                        var.sym,
                    );
                    self.databuf.borrow_mut().append_char(descriptor);
                    self.databuf.borrow_mut().append_char(var.reg);
                    let sym_ty = ctx.sym(var.sym).ty.expect("local variable type");
                    if self.needs_local_variable_type_entry(ctx, sym_ty) {
                        n_generic_vars += 1;
                    }
                }
            }
            self.end_attr(alen_idx);
            acount += 1;
            if n_generic_vars > 0 {
                let attr_name = self.names.LocalVariableTypeTable.clone();
                alen_idx = self.write_attr(ctx, attr_name);
                self.databuf.borrow_mut().append_char(n_generic_vars);
                let mut count = 0;
                for i in 0..var_buffer_size {
                    let var = match var_buffer.get(i as usize).and_then(|v| v.clone()) {
                        Some(var) => var,
                        None => continue,
                    };
                    let sym_ty = ctx.sym(var.sym).ty.expect("local variable type");
                    if !self.needs_local_variable_type_entry(ctx, sym_ty) {
                        continue;
                    }
                    for r in &var.alive_ranges {
                        self.databuf.borrow_mut().append_char(r.start_pc);
                        self.databuf.borrow_mut().append_char(r.length);
                        let name = ctx.sym(var.sym).name.clone();
                        let name_index = self.pool_writer.borrow_mut().put_name(name);
                        self.databuf.borrow_mut().append_char(name_index);
                        let sig = self.pool_writer.borrow_mut().put_signature(
                            ctx,
                            &self.syms,
                            var.sym,
                        );
                        self.databuf.borrow_mut().append_char(sig);
                        self.databuf.borrow_mut().append_char(var.reg);
                        count += 1;
                    }
                }
                assert!(count == n_generic_vars, "AssertionError: generic variable count");
                self.end_attr(alen_idx);
                acount += 1;
            }
        }
        if stack_map_buffer_size > 0 {
            if self.debug_stackmap {
                eprintln!("Stack map for {}", symbol_display(ctx, meth));
            }
            let attr_name = stack_map.get_attribute_name(&self.names);
            let alen_idx = self.write_attr(ctx, attr_name);
            self.write_stack_map(
                ctx,
                stack_map,
                &stack_map_buffer,
                &stack_map_table_buffer,
                stack_map_buffer_size,
            );
            self.end_attr(alen_idx);
            acount += 1;
        }
        let raw_type = ctx.sym(meth).get_raw_type_attributes();
        acount += self.write_type_annotations(ctx, raw_type, true);
        self.end_attrs(acount_idx, acount);
    }

    /// Return whether a local variable needs a type table entry.
    fn needs_local_variable_type_entry(&self, ctx: &mut CompilerContext, t: TypeId) -> bool {
        let syms = Rc::clone(&self.syms);
        let erased = self.types.erasure(ctx, &*syms, t);
        !self
            .types
            .is_same_type(ctx, &*syms, t, erased)
            && self.check.check_denotable(ctx, &*syms, t)
    }

    /// Write the StackMap attribute of a method.
    fn write_stack_map(
        &self,
        ctx: &mut CompilerContext,
        stack_map_format: StackMapFormat,
        stack_map_buffer: &[StackMapFrame],
        stack_map_table_buffer: &[StackMapTableFrame],
        nframes: i32,
    ) {
        if self.debug_stackmap {
            eprintln!(" nframes = {}", nframes);
        }
        self.databuf.borrow_mut().append_char(nframes);
        match stack_map_format {
            StackMapFormat::CLDC => {
                for (i, frame) in stack_map_buffer
                    .iter()
                    .take(nframes as usize)
                    .enumerate()
                {
                    if self.debug_stackmap {
                        eprintln!("  {}: pc={}", i, frame.pc);
                    }
                    self.databuf.borrow_mut().append_char(frame.pc);
                    let mut local_count = 0;
                    let mut j = 0;
                    while j < frame.locals.len() {
                        let width = frame
                            .locals
                            .get(j)
                            .and_then(|t| *t)
                            .map(|t| crate::jvm::code::Code::width(crate::jvm::code::Code::typecode(ctx, t)))
                            .unwrap_or(1);
                        local_count += 1;
                        j += width.max(1) as usize;
                    }
                    if self.debug_stackmap {
                        eprintln!(" nlocals={}", local_count);
                    }
                    self.databuf.borrow_mut().append_char(local_count);
                    let mut j = 0;
                    while j < frame.locals.len() {
                        let t = frame.locals[j];
                        self.write_stack_map_type(ctx, t);
                        let width = t
                            .map(|t| crate::jvm::code::Code::width(crate::jvm::code::Code::typecode(ctx, t)))
                            .unwrap_or(1);
                        j += width.max(1) as usize;
                    }
                    let mut stack_count = 0;
                    let mut j = 0;
                    while j < frame.stack.len() {
                        let width = crate::jvm::code::Code::width(crate::jvm::code::Code::typecode(ctx, frame.stack[j]));
                        stack_count += 1;
                        j += width.max(1) as usize;
                    }
                    self.databuf.borrow_mut().append_char(stack_count);
                    let mut j = 0;
                    while j < frame.stack.len() {
                        let t = frame.stack[j];
                        self.write_stack_map_type(ctx, Some(t));
                        let width = crate::jvm::code::Code::width(crate::jvm::code::Code::typecode(ctx, t));
                        j += width.max(1) as usize;
                    }
                }
            }
            StackMapFormat::JSR202 => {
                for frame in stack_map_table_buffer.iter().take(nframes as usize) {
                    let frame = frame.clone();
                    frame.write(ctx, self);
                    if self.debug_stackmap {
                        eprintln!();
                    }
                }
            }
            StackMapFormat::NONE => panic!("AssertionError: Unexpected stackmap format value"),
        }
    }

    /// Write one verifier type into the stack map.
    fn write_stack_map_type(&self, ctx: &mut CompilerContext, t: Option<TypeId>) {
        let Some(t) = t else {
            if self.debug_stackmap {
                eprintln!("empty");
            }
            self.databuf.borrow_mut().append_byte(0);
            return;
        };
        match ctx.typ(t).get_tag() {
            crate::code::type_tag::TypeTag::BYTE
            | crate::code::type_tag::TypeTag::CHAR
            | crate::code::type_tag::TypeTag::SHORT
            | crate::code::type_tag::TypeTag::INT
            | crate::code::type_tag::TypeTag::BOOLEAN => {
                if self.debug_stackmap {
                    eprintln!("int");
                }
                self.databuf.borrow_mut().append_byte(1);
            }
            crate::code::type_tag::TypeTag::FLOAT => {
                if self.debug_stackmap {
                    eprintln!("float");
                }
                self.databuf.borrow_mut().append_byte(2);
            }
            crate::code::type_tag::TypeTag::DOUBLE => {
                if self.debug_stackmap {
                    eprintln!("double");
                }
                self.databuf.borrow_mut().append_byte(3);
            }
            crate::code::type_tag::TypeTag::LONG => {
                if self.debug_stackmap {
                    eprintln!("long");
                }
                self.databuf.borrow_mut().append_byte(4);
            }
            crate::code::type_tag::TypeTag::BOT => {
                if self.debug_stackmap {
                    eprintln!("null");
                }
                self.databuf.borrow_mut().append_byte(5);
            }
            crate::code::type_tag::TypeTag::CLASS
            | crate::code::type_tag::TypeTag::ARRAY
            | crate::code::type_tag::TypeTag::TYPEVAR => {
                if self.debug_stackmap {
                    eprintln!("object");
                }
                self.databuf.borrow_mut().append_byte(7);
                let syms = Rc::clone(&self.syms);
                let erased = self.types.erasure(ctx, &*syms, t);
                let index = self.pool_writer.borrow_mut().put_class(ctx, &*syms, erased);
                self.databuf.borrow_mut().append_char(index);
            }
            crate::code::type_tag::TypeTag::UNINITIALIZED_THIS => {
                if self.debug_stackmap {
                    eprintln!("uninit_this");
                }
                self.databuf.borrow_mut().append_byte(6);
            }
            crate::code::type_tag::TypeTag::UNINITIALIZED_OBJECT => {
                let offset = crate::jvm::uninitialized_type::offset(ctx, t).expect("uninit offset");
                self.databuf.borrow_mut().append_byte(8);
                if self.debug_stackmap {
                    eprintln!("uninit_object@{}", offset);
                }
                self.databuf.borrow_mut().append_char(offset);
            }
            other => panic!("AssertionError: unexpected stack map type {:?}", other),
        }
    }

    /// Write the fields of a class.
    pub fn write_fields(&self, ctx: &mut CompilerContext, s: crate::ids::ScopeId) {
    let scope_snapshot = ctx.scope(s).clone();
        let symbols = scope_snapshot.get_symbols(
            s,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
            ctx,
        );
        let mut vars: Vec<SymbolId> = Vec::new();
        for sym in symbols {
            if ctx.sym(sym).kind == crate::code::kinds::Kind::VAR {
                vars.push(sym);
            }
        }
        for v in vars.into_iter().rev() {
            self.write_field(ctx, v);
        }
    }

    /// Write the methods of a class.
    pub fn write_methods(&self, ctx: &mut CompilerContext, s: crate::ids::ScopeId) {
        let scope_snapshot = ctx.scope(s).clone();
        let symbols = scope_snapshot.get_symbols(
            s,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
            ctx,
        );
        let mut methods: Vec<SymbolId> = Vec::new();
        for sym in symbols {
            if ctx.sym(sym).kind == crate::code::kinds::Kind::MTH
                && (ctx.sym(sym).flags_field & flags::HYPOTHETICAL) == 0
            {
                methods.push(sym);
            }
        }
        for m in methods.into_iter().rev() {
            self.write_method(ctx, m);
        }
    }
}

impl StackMapTableWriter for ClassWriter {
    fn append_byte(&self, b: i32) {
        self.databuf.borrow_mut().append_byte(b);
    }

    fn append_char(&self, c: i32) {
        self.databuf.borrow_mut().append_char(c);
    }

    fn debug_stackmap(&self) -> bool {
        self.debug_stackmap
    }

    fn debug_print(&self, text: &str) {
        eprintln!("{}", text);
    }

    fn write_stack_map_type(&self, ctx: &mut CompilerContext, t: Option<TypeId>) {
        ClassWriter::write_stack_map_type(self, ctx, t);
    }
}

/// Return whether a method is a lambda body.
fn is_lambda_method(ctx: &CompilerContext, m: SymbolId) -> bool {
    (ctx.sym(m).flags_field & flags::LAMBDA_METHOD) != 0
}

/// Return a printable description of a symbol.
fn symbol_display(ctx: &CompilerContext, sym: SymbolId) -> String {
    ctx.sym(sym).name.to_java_string().to_string_lossy()
}

impl ClassWriter {
    /// Emit the class file for a given class and return the class file name and
    /// the generated bytes.
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - c: the class symbol.
    ///
    /// # Returns
    ///   - the (relative class file name, class file bytes) pair.
    ///
    /// # Panics
    ///   - a class invariant is violated.
    pub fn write_class(
        &self,
        ctx: &mut CompilerContext,
        c: SymbolId,
    ) -> Result<(String, Vec<u8>), ClassWriterError> {
        let (owner, c_name, flatname) = (
            ctx.sym(c).owner,
            ctx.sym(c).name.clone(),
            class_flat_name(ctx, c),
        );
        let owner_kind = owner.map(|owner| ctx.sym(owner).kind);
        let name = if owner_kind == Some(crate::code::kinds::Kind::MDL) {
            c_name.to_java_string().to_string_lossy()
        } else {
            flatname
        };
        let mut out: Vec<u8> = Vec::new();
        self.write_class_file(ctx, &mut out, c)?;
        Ok((name, out))
    }

    /// Write the class of c to the given output buffer.
    pub fn write_class_file(
        &self,
        ctx: &mut CompilerContext,
        out: &mut Vec<u8>,
        c: SymbolId,
    ) -> Result<(), ClassWriterError> {
        let c_ty = ctx.sym(c).ty.expect("class type");
        let syms = Rc::clone(&self.syms);
        let supertype = self.types.supertype(ctx, &*syms, c_ty, None);
        let interfaces = self.types.interfaces(ctx, &*syms, c_ty);
        let typarams =
            crate::code::types::get_type_arguments_completed(ctx, &*syms, c_ty, None);
        let owner = ctx.sym(c).owner.expect("class owner");
        let owner_kind = ctx.sym(owner).kind;
        let c_flags = ctx.sym(c).flags();
        let mut flags_value;
        if owner_kind == crate::code::kinds::Kind::MDL {
            flags_value = flags::ACC_MODULE;
        } else {
            flags_value = adjust_flags(c_flags & !flags::DEFAULT);
            if (flags_value & flags::PROTECTED) != 0 {
                flags_value |= flags::PUBLIC;
            }
            // Java: flags = flags & ClassFlags & ~STRICTFP (ClassWriter.java:
            // 1539-1542); ClassFlags includes ANNOTATION, StandardFlags does not.
            flags_value &= flags::ClassFlags & !flags::STRICTFP;
            if (flags_value & flags::INTERFACE) == 0 {
                flags_value |= flags::ACC_SUPER;
            }
        }
        if self.dump_class_modifiers {
            eprintln!();
            eprintln!("CLASSFILE  {}", qualified_name(ctx, c));
            eprintln!("---{}", ClassWriter::flag_names(flags_value as i64));
            let _ = flags_value;
        }
        self.databuf.borrow_mut().reset();
        self.databuf.borrow_mut().append_char(flags_value);
        let syms = Rc::clone(&self.syms);
        if owner_kind == crate::code::kinds::Kind::MDL {
            let unnamed = match &ctx.sym(owner).kind_data {
                SymbolKind::ModuleSymbol { unnamed_package, .. } => *unnamed_package,
                _ => None,
            };
            let module_info = match unnamed {
                Some(unnamed) => create_module_info_class(ctx, &self.names, unnamed),
                None => c,
            };
            let index = self.pool_writer.borrow_mut().put_class_symbol(
                ctx,
                &*syms,
                module_info,
            );
            self.databuf.borrow_mut().append_char(index);
        } else {
            let index = self.pool_writer.borrow_mut().put_class_symbol(ctx, &*syms, c);
            self.databuf.borrow_mut().append_char(index);
        }
        let super_index = if ctx.typ(supertype).has_tag(crate::code::type_tag::TypeTag::CLASS) {
            let tsym = ctx.typ(supertype).tsym.expect("super type symbol");
            self.pool_writer
                .borrow_mut()
                .put_class_symbol(ctx, &*syms, tsym)
        } else {
            0
        };
        self.databuf.borrow_mut().append_char(super_index);
        let mut interface_tsyms: Vec<SymbolId> = Vec::new();
        self.databuf.borrow_mut().append_char(interfaces.length());
        let mut l = interfaces.clone();
        for ty in &l {
        let tsym = ctx.typ(ty).tsym.expect("interface type symbol");
        interface_tsyms.push(tsym);
        let index = self
            .pool_writer
            .borrow_mut()
            .put_class_symbol(ctx, &*syms, tsym);
        self.databuf.borrow_mut().append_char(index);
        }
        let members = ctx.sym(c).members().expect("class members");
        let scope_snapshot = ctx.scope(members).clone();
        let symbols = scope_snapshot.get_symbols(
            members,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
            ctx,
        );
        let mut fields_count = 0;
        let mut methods_count = 0;
        let mut inner_classes: Vec<SymbolId> = Vec::new();
        for sym in &symbols {
            let sym = *sym;
            match ctx.sym(sym).kind {
                crate::code::kinds::Kind::VAR => fields_count += 1,
                crate::code::kinds::Kind::MTH => {
                    if (ctx.sym(sym).flags_field & flags::HYPOTHETICAL) == 0 {
                        methods_count += 1;
                    }
                }
                crate::code::kinds::Kind::TYP => inner_classes.push(sym),
                _ => panic!("AssertionError: unexpected class member"),
            }
        }
        for inner in inner_classes {
            self.pool_writer
                .borrow_mut()
                .enter_inner(ctx, inner);
        }
        let trans_local = match &ctx.sym(c).kind_data {
            SymbolKind::ClassSymbol { trans_local, .. } => trans_local.clone(),
            _ => List::nil(),
        };
        let mut current = trans_local;
        for local in &current {
        self.pool_writer.borrow_mut().enter_inner(ctx, local);
        }
        self.databuf.borrow_mut().append_char(fields_count);
        self.write_fields(ctx, members);
        self.databuf.borrow_mut().append_char(methods_count);
        self.write_methods(ctx, members);
        let acount_idx = self.begin_attrs();
        let mut acount = 0;
        let mut sig_req = typarams.length() != 0
            || crate::code::types::allparams_of(ctx, supertype).length() != 0;
        let mut l = interfaces;
        while !sig_req && l.non_empty() {
            let ty = *l.head();
            l = l.tail();
            sig_req = crate::code::types::allparams_of(ctx, ty).length() != 0;
        }
        if sig_req {
            let attr_name = self.names.Signature.clone();
            let alen_idx = self.write_attr(ctx, attr_name);
            let index = self.pool_writer.borrow_mut().put_signature(
                ctx,
                &self.syms,
                c,
            );
            self.databuf.borrow_mut().append_char(index);
            self.end_attr(alen_idx);
            acount += 1;
        }
        let sourcefile = class_sourcefile(ctx, c);
        if let Some(source_file) = sourcefile.as_ref()
            && self.emit_source_file
        {
            let attr_name = self.names.SourceFile.clone();
            let alen_idx = self.write_attr(ctx, attr_name);
            let simple = simple_name(source_file);
            let name = self.names.from_string(&simple);
            let index = self.pool_writer.borrow_mut().put_name(name);
            self.databuf.borrow_mut().append_char(index);
            self.end_attr(alen_idx);
            acount += 1;
        }
        if self.gen_crt {
            // SourceID attribute
            let attr_name = self.names.SourceID.clone();
            let alen_idx = self.write_attr(ctx, attr_name);
            let modified = last_modified(&sourcefile);
            let name = self.names.from_string(
                &crate::util::java_string::JavaString::from(modified.to_string().as_str()),
            );
            let index = self.pool_writer.borrow_mut().put_name(name);
            self.databuf.borrow_mut().append_char(index);
            self.end_attr(alen_idx);
            acount += 1;
            // CompilationID attribute
            let attr_name = self.names.CompilationID.clone();
            let alen_idx = self.write_attr(ctx, attr_name);
            let now = current_time_millis();
            let name = self.names.from_string(
                &crate::util::java_string::JavaString::from(now.to_string().as_str()),
            );
            let index = self.pool_writer.borrow_mut().put_name(name);
            self.databuf.borrow_mut().append_char(index);
            self.end_attr(alen_idx);
            acount += 1;
        }
        acount += self.write_flag_attrs(ctx, c_flags);
        let raw = ctx.sym(c).get_raw_attributes();
        acount += self.write_java_annotations(ctx, raw);
        let raw_type = ctx.sym(c).get_raw_type_attributes();
        acount += self.write_type_annotations(ctx, raw_type, false);
        acount += self.write_enclosing_method_attribute(ctx, c);
        if owner_kind == crate::code::kinds::Kind::MDL {
            acount += self.write_module_attribute(ctx, c);
            acount += self.write_flag_attrs(ctx, ctx.sym(owner).flags() & !(flags::DEPRECATED as i64));
        }
        acount += self.write_extra_class_attributes(ctx, c);
        acount += self.write_extra_attributes(ctx, c);
        self.poolbuf.borrow_mut().reset();
        self.poolbuf.borrow_mut().append_int(class_file::JAVA_MAGIC);
        let use_preview = self.preview.is_enabled()
            && sourcefile
                .as_ref()
                .map(|file| self.preview.uses_preview(file))
                .unwrap_or(false);
        if use_preview {
            self.poolbuf.borrow_mut().append_char(class_file::PREVIEW_MINOR_VERSION);
        } else {
            self.poolbuf.borrow_mut().append_char(self.target.minor_version());
        }
        self.poolbuf.borrow_mut().append_char(self.target.major_version());
        if owner_kind != crate::code::kinds::Kind::MDL && self.target.has_nestmate_access() {
            acount += self.write_nest_members_if_needed(ctx, c);
            acount += self.write_nest_host_if_needed(ctx, c);
        }
        if ctx.sym(c).is_record() {
            acount += self.write_record_attribute(ctx, c);
        }
        if self.target.has_sealed_classes() {
            acount += self.write_permitted_subclasses_if_needed(ctx, c);
        }
        let has_bootstrap = !self.pool_writer.borrow().bootstrap_methods.is_empty();
        if has_bootstrap {
            self.write_bootstrap_methods(ctx);
            acount += 1;
        }
        let has_inner = !self.pool_writer.borrow().inner_classes.is_empty();
        if has_inner {
            self.write_inner_classes(ctx);
            acount += 1;
        }
        self.end_attrs(acount_idx, acount);
        out.extend_from_slice(&self.poolbuf.borrow().elems[..self.poolbuf.borrow().length as usize]);
        {
            let writer = self.pool_writer.borrow();
            writer
                .write_pool(out)
                .map_err(|error| match error {
                    PoolWriterError::PoolOverflow => ClassWriterError::PoolOverflow,
                    PoolWriterError::StringOverflow(text) => {
                        ClassWriterError::StringOverflow(text)
                    }
                })?;
        }
        let signature_error = self.pool_writer.borrow_mut().take_signature_error();
        self.pool_writer.borrow_mut().reset();
        if let Some(type_) = signature_error {
            return Err(ClassWriterError::InvalidSignature(type_));
        }
        out.extend_from_slice(&self.databuf.borrow().elems[..self.databuf.borrow().length as usize]);
        Ok(())
    }

    /// Allow subclasses to write additional class attributes.
    pub fn write_extra_class_attributes(
        &self,
        _ctx: &mut CompilerContext,
        _c: SymbolId,
    ) -> i32 {
        0
    }
}

