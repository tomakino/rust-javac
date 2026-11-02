//! Native-header writer: emits the C header for a class exposing native
//! methods and/or fields annotated with `@Native`, ported 1:1 from
//! JNIWriter.java.
//!
//!
//! - the Java compiler instance and Context are replaced by explicit
//!   CompilerContext/Symtab arguments on every method (registered adapter);
//! - the Java PrintWriter is a String buffer here and the finished header is
//!   written through FileObject.openWriter (the output bytes are identical);
//!   registered adapter;
//! - multiModuleMode is a public field like upstream.

use std::cell::RefCell;
use std::rc::Rc;

use javac_tools::java_io::Writer;
use javac_tools::tools::file_object::FileObject;
use javac_tools::tools::java_file_manager::{
    FileObjectRef, JavaFileManager, Location, LocationRef,
};
use javac_tools::tools::standard_location::StandardLocation;
use javac_strings::JavaString;

use crate::code::flags;
use crate::code::kinds::Kind;
use crate::code::symbol::{symbol_is_directly_or_indirectly_local, symbol_packge, SymbolKind};
use crate::ids::SymbolId;
use crate::code::symtab::Symtab;
use crate::code::type_::TypeKindData;
use crate::code::type_tag::TypeTag;
use crate::code::types::Types;
use crate::compiler_context::CompilerContext;
use crate::util::constants::ConstantValue;
use crate::util::jc_diagnostic::DiagnosticArg;
use crate::util::log::Log;
use crate::util::options::Options;

/// The result of writing one native header.
///
/// The upstream method throws IOException; the port reports the equivalent
/// failure as a String because the ported FileObject API returns Tool errors.
type HeaderResult<T> = Result<T, String>;

/// This class provides operations to write native header files for classes.
///
/// Java: public class JNIWriter (JNIWriter.java:66).
pub struct JNIWriter {
    /// Access to files (Java: private final JavaFileManager fileManager).
    file_manager: Rc<dyn JavaFileManager>,
    /// Java: private final Log log.
    log: Rc<Log>,
    /// Java: Types types (lazyInit JNIWriter.java:120-126).
    types: Rc<Types>,
    /// Switch: verbose output (Java: private boolean verbose).
    verbose: bool,
    /// Switch: check all nested classes of top level class (javah:full).
    check_all: bool,
    /// If true, header files are written in module-specific subdirectories of
    /// the NATIVE_HEADER_OUTPUT location.
    pub multi_module_mode: bool,
}

impl JNIWriter {
    /// Java: private JNIWriter(Context context) (JNIWriter.java:108-118).
    ///
    /// # Arguments
    ///   - options: the option table;
    ///   - file_manager: the shared file manager;
    ///   - log: the diagnostic log.
    ///
    /// # Returns
    ///   - the writer.
    ///
    /// # Panics
    ///   - none.
    pub fn new(
        options: &Options,
        file_manager: Rc<dyn JavaFileManager>,
        log: Rc<Log>,
        types: Rc<Types>,
    ) -> JNIWriter {
        let verbose = options.is_set_option(crate::main::option::Option::VERBOSE);
        let check_all = options.is_set("javah:full");
        JNIWriter {
            file_manager,
            log,
            types,
            verbose,
            check_all,
            multi_module_mode: false,
        }
    }

    /// Java: public boolean needsHeader(ClassSymbol c) (JNIWriter.java:144-151).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - c: the class symbol.
    ///
    /// # Returns
    ///   - whether a native header must be emitted for the class.
    ///
    /// # Panics
    ///   - none.
    pub fn needs_header(&self, ctx: &mut CompilerContext, syms: &Symtab, c: SymbolId) -> bool {
        if symbol_is_directly_or_indirectly_local(ctx, c) || has_flag(ctx, c, flags::SYNTHETIC as i64) {
            return false;
        }
        if self.check_all {
            match outermost_class(ctx, c) {
                Some(outer) => self.needs_header_impl(ctx, syms, outer, true),
                None => false,
            }
        } else {
            self.needs_header_impl(ctx, syms, c, false)
        }
    }

    /// Java: private boolean needsHeader(ClassSymbol c, boolean
    /// checkNestedClasses) (JNIWriter.java:153-172).
    fn needs_header_impl(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        c: SymbolId,
        check_nested_classes: bool,
    ) -> bool {
        if symbol_is_directly_or_indirectly_local(ctx, c) || has_flag(ctx, c, flags::SYNTHETIC as i64) {
            return false;
        }
        let members = class_members(ctx, c);
        let native_header_tsym = ctx.typ(syms.native_header_type).tsym;
        for sym in &members {
            if ctx.sym(*sym).kind == Kind::MTH && has_flag(ctx, *sym, flags::NATIVE as i64) {
                return true;
            }
            for attribute in ctx.sym(*sym).get_declaration_attributes().iter() {
                if let Some(ty) = ctx.attr(attribute).ty
                    && ctx.typ(ty).tsym == native_header_tsym
                {
                    return true;
                }
            }
        }
        if check_nested_classes {
            for sym in &members {
                if ctx.sym(*sym).kind == Kind::TYP
                    && self.needs_header_impl(ctx, syms, *sym, true)
                {
                    return true;
                }
            }
        }
        false
    }

    /// Java: public FileObject write(ClassSymbol c) (JNIWriter.java:177-205).
    ///
    /// # Arguments
    ///   - ctx/syms: explicit services;
    ///   - c: the class symbol.
    ///
    /// # Returns
    ///   - the created header file object.
    ///
    /// # Panics
    ///   - none.
    pub fn write(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        c: SymbolId,
    ) -> HeaderResult<FileObjectRef> {
        let class_name = class_flatname(ctx, c).to_string_lossy();
        let native_header_output =
            StandardLocation::location_for(&JavaString::from("NATIVE_HEADER_OUTPUT"));
        let out_locn: LocationRef = if self.multi_module_mode {
            let msym = module_of_class(ctx, c).expect("AssertionError: class module");
            let module_name = ctx.sym(msym).name.to_java_string();
            self.file_manager
                .get_location_for_module(&*native_header_output, &module_name)
                .map_err(|error| format!("cannot locate the native header module: {error:?}"))?
        } else {
            Rc::clone(&native_header_output)
        };
        let mut relative = String::new();
        for ch in class_name.chars() {
            if ch == '.' || ch == '$' {
                relative.push('_');
            } else {
                relative.push(ch);
            }
        }
        relative.push_str(".h");
        let out_file = self
            .file_manager
            .get_file_for_output(
                &*out_locn,
                &JavaString::from(""),
                &JavaString::from(relative.as_str()),
                None,
            )
            .map_err(|error| format!("cannot create the native header: {error:?}"))?;
        let header = self.write_string(ctx, syms, c)?;
        let mut writer = out_file
            .open_writer()
            .map_err(|error| format!("cannot open the native header: {error:?}"))?;
        let units: Vec<u16> = header.encode_utf16().collect();
        if let Err(error) = writer.write(&units) {
            let _ = out_file.delete();
            return Err(format!("cannot write the native header: {error:?}"));
        }
        let _ = writer.flush();
        let _ = writer.close();
        if self.verbose {
            let name = out_file.get_name();
            let arg: DiagnosticArg = Rc::new(name);
            self.log
                .print_verbose("wrote.file", vec![arg]);
        }
        Ok(out_file)
    }

    /// Java: public void write(PrintWriter out, ClassSymbol sym)
    /// (JNIWriter.java:207-224).
    fn write_string(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        sym: SymbolId,
    ) -> HeaderResult<String> {
        let mut out = String::new();
        let cname = encode_java_string(&class_fullname(ctx, sym), EncoderType::CLASS);
        self.file_top(&mut out);
        self.includes(&mut out);
        self.guard_begin(&mut out, &cname);
        self.cpp_guard_begin(&mut out);
        self.write_statics(ctx, syms, &mut out, sym)?;
        self.write_methods(ctx, syms, &mut out, sym, &cname)?;
        self.cpp_guard_end(&mut out);
        self.guard_end(&mut out);
        Ok(out)
    }

    /// Java: protected void writeStatics(PrintWriter out, ClassSymbol sym)
    /// (JNIWriter.java:225-248).
    fn write_statics(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        out: &mut String,
        sym: SymbolId,
    ) -> HeaderResult<()> {
        let mut clist: Vec<SymbolId> = Vec::new();
        let mut current = Some(sym);
        while let Some(cd) = current {
            clist.push(cd);
            current = self.class_superclass(ctx, syms, cd);
        }
        clist.reverse();
        for cd in clist {
            for i in class_members(ctx, cd) {
                if has_flag(ctx, i, flags::FINAL as i64)
                    && ctx.sym(i).is_static(ctx)
                    && ctx.sym(i).kind == Kind::VAR
                    && let Some(value) = symbol_constant_value(ctx, i)
                {
                    self.print_static_defines(ctx, out, sym, i, &value);
                }
            }
        }
        Ok(())
    }

    /// Java: static void printStaticDefines(PrintWriter out,
    /// Pair<ClassSymbol, VarSymbol> p) (JNIWriter.java:249-294).
    fn print_static_defines(
        &self,
        ctx: &CompilerContext,
        out: &mut String,
        cls: SymbolId,
        f: SymbolId,
        value: &ConstantValue,
    ) {
        let Some(ty) = ctx.sym(f).ty else { return };
        let value_str = match ctx.typ(ty).get_tag() {
            TypeTag::BOOLEAN => match value {
                ConstantValue::Boolean(v) => Some(if *v { "1L".to_string() } else { "0L".to_string() }),
                _ => None,
            },
            TypeTag::BYTE => match value {
                ConstantValue::Byte(v) => Some(format!("{v}L")),
                _ => None,
            },
            TypeTag::SHORT => match value {
                ConstantValue::Short(v) => Some(format!("{v}L")),
                _ => None,
            },
            TypeTag::INT => match value {
                ConstantValue::Integer(v) => Some(format!("{v}L")),
                _ => None,
            },
            TypeTag::LONG => match value {
                ConstantValue::Long(v) => Some(format!(
                    "{v}{}",
                    if std::env::consts::OS == "windows" { "i64" } else { "LL" }
                )),
                _ => None,
            },
            TypeTag::CHAR => match value {
                ConstantValue::Character(ch) => Some(format!("{}L", (*ch as i32) & 0xffff)),
                _ => None,
            },
            TypeTag::FLOAT => match value {
                ConstantValue::Float(v) => {
                    if v.is_infinite() {
                        Some(format!("{}Inff", if *v < 0.0 { "-" } else { "" }))
                    } else {
                        Some(format!("{}f", javac_floating::to_java_string_float(*v)))
                    }
                }
                _ => None,
            },
            TypeTag::DOUBLE => match value {
                ConstantValue::Double(v) => {
                    if v.is_infinite() {
                        Some(format!("{}InfD", if *v < 0.0 { "-" } else { "" }))
                    } else {
                        Some(javac_floating::to_java_string_double(*v))
                    }
                }
                _ => None,
            },
            _ => None,
        };
        if let Some(value_str) = value_str {
            out.push_str("#undef ");
            let cname = encode_java_string(&class_qualified_name(ctx, cls), EncoderType::CLASS);
            let fname = encode_java_string(
                &ctx.sym(f).name.to_java_string(),
                EncoderType::FIELDSTUB,
            );
            out.push_str(&format!("{cname}_{fname}\n"));
            out.push_str(&format!("#define {cname}_"));
            out.push_str(&format!("{fname} {value_str}\n"));
        }
    }

    /// Java: protected void writeMethods(PrintWriter out, ClassSymbol sym,
    /// String cname) (JNIWriter.java:295-329).
    fn write_methods(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        out: &mut String,
        sym: SymbolId,
        cname: &str,
    ) -> HeaderResult<()> {
        let class_methods = class_members(ctx, sym);
        for md in &class_methods {
            if !has_flag(ctx, *md, flags::NATIVE as i64) {
                continue;
            }
            let types = TypeSignature;
            let method_name = ctx.sym(*md).name.to_java_string();
            let mut is_overloaded = false;
            for md2 in &class_methods {
                if md2 != md
                    && method_name.to_string_lossy()
                        == ctx.sym(*md2).name.to_java_string().to_string_lossy()
                    && has_flag(ctx, *md2, flags::NATIVE as i64)
                {
                    is_overloaded = true;
                }
            }
            let md_ty = ctx.sym(*md).ty.expect("AssertionError: method type");
            let signature = types.get_signature(&self.types, ctx, syms, md_ty)?;
            out.push_str("/*\n");
            out.push_str(&format!(" * Class:     {cname}\n"));
            out.push_str(&format!(
                " * Method:    {}\n",
                encode_java_string(&method_name, EncoderType::FIELDSTUB)
            ));
            out.push_str(&format!(" * Signature: {signature}\n"));
            out.push_str(" */\n");
            let return_type = ctx.typ(md_ty).get_return_type(ctx).expect("method return type");
            let erased_return = self.types.erasure(ctx, syms, return_type);
            let jni_return = self.jni_type(ctx, syms, erased_return);
            let encoded_method = self.encode_method(ctx, syms, *md, sym, is_overloaded)?;
            out.push_str(&format!(
                "JNIEXPORT {jni_return} JNICALL {encoded_method}\n"
            ));
            out.push_str("  (JNIEnv *, ");
            out.push_str(if ctx.sym(*md).is_static(ctx) { "jclass" } else { "jobject" });
            let parameter_types = ctx.typ(md_ty).get_parameter_types(ctx);
            for arg in parameter_types.iter() {
                out.push_str(", ");
                let erased_arg = self.types.erasure(ctx, syms, arg);
                out.push_str(&self.jni_type(ctx, syms, erased_arg));
            }
            out.push_str(");\n");
            out.push_str("\n");
        }
        Ok(())
    }

    /// Java: protected final String jniType(Type t) (JNIWriter.java:330-374).
    fn jni_type(&self, ctx: &mut CompilerContext, syms: &Symtab, t: crate::ids::TypeId) -> String {
        jni_type_of(&self.types, ctx, syms, t)
    }
}

/// The body of JNIWriter.jniType (JNIWriter.java:330-374); extracted so the
/// primitive mapping can be unit-tested without a compiler pipeline.
fn jni_type_of(
    types: &Types,
    ctx: &mut CompilerContext,
    syms: &Symtab,
    t: crate::ids::TypeId,
) -> String {
        match ctx.typ(t).get_tag() {
            TypeTag::ARRAY => {
                let component = match &ctx.typ(t).kind {
                    TypeKindData::ArrayType { elemtype, .. } => {
                        elemtype.expect("array element type")
                    }
                    _ => crate::util::assert::error(),
                };
                match ctx.typ(component).get_tag() {
                    TypeTag::BOOLEAN => "jbooleanArray",
                    TypeTag::BYTE => "jbyteArray",
                    TypeTag::CHAR => "jcharArray",
                    TypeTag::SHORT => "jshortArray",
                    TypeTag::INT => "jintArray",
                    TypeTag::LONG => "jlongArray",
                    TypeTag::FLOAT => "jfloatArray",
                    TypeTag::DOUBLE => "jdoubleArray",
                    TypeTag::ARRAY | TypeTag::CLASS => "jobjectArray",
                    _ => crate::util::assert::error(),
                }
                .to_string()
            }
            TypeTag::VOID => "void".to_string(),
            TypeTag::BOOLEAN => "jboolean".to_string(),
            TypeTag::BYTE => "jbyte".to_string(),
            TypeTag::CHAR => "jchar".to_string(),
            TypeTag::SHORT => "jshort".to_string(),
            TypeTag::INT => "jint".to_string(),
            TypeTag::LONG => "jlong".to_string(),
            TypeTag::FLOAT => "jfloat".to_string(),
            TypeTag::DOUBLE => "jdouble".to_string(),
            TypeTag::CLASS => {
                let tsym = ctx.typ(t).tsym;
                if tsym == ctx.typ(syms.string_type).tsym {
                    "jstring".to_string()
                } else if types.is_assignable(ctx, syms, t, syms.throwable_type) {
                    "jthrowable".to_string()
                } else if types.is_assignable(ctx, syms, t, syms.class_type) {
                    "jclass".to_string()
                } else {
                    "jobject".to_string()
                }
            }
            _ => crate::util::assert::error(),
        }
}

impl JNIWriter {
    fn file_top(&self, out: &mut String) {
        out.push_str("/* DO NOT EDIT THIS FILE - it is machine generated */\n");
    }

    fn includes(&self, out: &mut String) {
        out.push_str("#include <jni.h>\n");
    }

    fn cpp_guard_begin(&self, out: &mut String) {
        out.push_str("#ifdef __cplusplus\n");
        out.push_str("extern \"C\" {\n");
        out.push_str("#endif\n");
    }

    fn cpp_guard_end(&self, out: &mut String) {
        out.push_str("#ifdef __cplusplus\n");
        out.push_str("}\n");
        out.push_str("#endif\n");
    }

    fn guard_begin(&self, out: &mut String, cname: &str) {
        out.push_str(&format!("/* Header for class {cname} */\n"));
        out.push('\n');
        out.push_str(&format!("#ifndef _Included_{cname}\n"));
        out.push_str(&format!("#define _Included_{cname}\n"));
    }

    fn guard_end(&self, out: &mut String) {
        out.push_str("#endif\n");
    }

}

impl JNIWriter {
    /// Java: String encodeMethod(Symbol msym, ClassSymbol clazz,
    /// boolean isOverloaded) (JNIWriter.java:410-424).
    fn encode_method(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        msym: SymbolId,
        clazz: SymbolId,
        is_overloaded: bool,
    ) -> HeaderResult<String> {
        let mut result = String::new();
        result.push_str("Java_");
        result.push_str(&encode_java_string(
            &class_flatname(ctx, clazz),
            EncoderType::JNI,
        ));
        result.push('_');
        result.push_str(&encode_java_string(
            &ctx.sym(msym).name.to_java_string(),
            EncoderType::JNI,
        ));
        if is_overloaded {
            let type_sig = TypeSignature;
            let msym_ty = ctx.sym(msym).ty.expect("AssertionError: method type");
            let sig = type_sig.get_parameter_signature(&self.types, ctx, syms, msym_ty, true)?;
            result.push_str("__");
            result.push_str(&encode_java_string(
                &JavaString::from(sig.as_str()),
                EncoderType::JNI,
            ));
        }
        Ok(result)
    }

    /// Java: private boolean needsHeader / getSuperclass walk
    /// (JNIWriter.java:226-229).
    fn class_superclass(
        &self,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        class: SymbolId,
    ) -> Option<SymbolId> {
        let ty = ctx.sym(class).ty?;
        let super_type = self.types.supertype(ctx, syms, ty, None);
        if ctx.typ(super_type).get_tag() != TypeTag::CLASS {
            return None;
        }
        ctx.typ(super_type).tsym
    }
}

/// Java: static enum EncoderType (JNIWriter.java:426-432).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EncoderType {
    /// Field type encoding for class names.
    CLASS,
    /// Static field stub encoding.
    FIELDSTUB,
    /// Field encoding.
    FIELD,
    /// JNI method name encoding.
    JNI,
    /// Method signature encoding.
    SIGNATURE,
}

/// Java: static String encode(CharSequence name, EncoderType mtype)
/// (JNIWriter.java:434-488).
fn encode_java_string(name: &JavaString, mtype: EncoderType) -> String {
    let units = name.as_units();
    let mut result = String::new();
    for &unit in units {
        if isalnum(unit) {
            result.push(char::from_u32(unit as u32).unwrap_or('?'));
            continue;
        }
        match mtype {
            EncoderType::CLASS => match char::from_u32(unit as u32).unwrap_or('?') {
                '.' | '_' => result.push('_'),
                '$' => result.push_str("__"),
                ch => result.push_str(&encode_char(unit, ch)),
            },
            EncoderType::JNI => match char::from_u32(unit as u32).unwrap_or('?') {
                '/' | '.' => result.push('_'),
                '_' => result.push_str("_1"),
                ';' => result.push_str("_2"),
                '[' => result.push_str("_3"),
                ch => result.push_str(&encode_char(unit, ch)),
            },
            EncoderType::SIGNATURE => {
                if isprint(unit) {
                    result.push(char::from_u32(unit as u32).unwrap_or('?'));
                } else {
                    result.push_str(&encode_char(unit, '?'));
                }
            }
            EncoderType::FIELDSTUB => {
                let ch = char::from_u32(unit as u32).unwrap_or('?');
                if ch == '_' {
                    result.push(ch);
                } else {
                    result.push_str(&encode_char(unit, ch));
                }
            }
            EncoderType::FIELD => {
                let ch = char::from_u32(unit as u32).unwrap_or('?');
                result.push_str(&encode_char(unit, ch));
            }
        }
    }
    result
}

/// Java: static String encodeChar(char ch) (JNIWriter.java:490-502).
fn encode_char(unit: u16, _ch: char) -> String {
    let hex = format!("{unit:x}");
    let mut result = String::from("_");
    let nzeros = 5usize.saturating_sub(hex.len());
    for _ in 0..nzeros {
        result.push('0');
    }
    result.push_str(&hex);
    result
}

/// Java: private static boolean isalnum(char ch) (JNIWriter.java:504-510).
fn isalnum(unit: u16) -> bool {
    unit <= 0x7f
        && ((0x41..=0x5a).contains(&unit)
            || (0x61..=0x7a).contains(&unit)
            || (0x30..=0x39).contains(&unit))
}

/// Java: private static boolean isprint(char ch) (JNIWriter.java:512-515).
fn isprint(unit: u16) -> bool {
    (32..=126).contains(&unit)
}

/// Java: private static class TypeSignature (JNIWriter.java:517-628).
struct TypeSignature;

impl TypeSignature {
    /// Java: StringBuilder getParameterSignature(Type mType,
    /// boolean useFlatname) (JNIWriter.java:545-552).
    fn get_parameter_signature(
        &self,
        types: &Types,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        m_type: crate::ids::TypeId,
        use_flatname: bool,
    ) -> HeaderResult<String> {
        let mut result = String::new();
        for p_type in ctx.typ(m_type).get_parameter_types(ctx).iter() {
            result.push_str(&self.get_jvm_signature(types, ctx, syms, p_type, use_flatname)?);
        }
        Ok(result)
    }

    /// Java: StringBuilder getReturnSignature(Type mType)
    /// (JNIWriter.java:554-557).
    fn get_return_signature(
        &self,
        types: &Types,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        m_type: crate::ids::TypeId,
    ) -> HeaderResult<String> {
        let return_type = ctx
            .typ(m_type)
            .get_return_type(ctx)
            .expect("AssertionError: method return type");
        self.get_jvm_signature(types, ctx, syms, return_type, false)
    }

    /// Java: StringBuilder getSignature(Type mType) (JNIWriter.java:559-564).
    fn get_signature(
        &self,
        types: &Types,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        m_type: crate::ids::TypeId,
    ) -> HeaderResult<String> {
        let mut result = String::new();
        result.push('(');
        result.push_str(&self.get_parameter_signature(types, ctx, syms, m_type, false)?);
        result.push(')');
        result.push_str(&self.get_return_signature(types, ctx, syms, m_type)?);
        Ok(result)
    }

    /// Java: StringBuilder getJvmSignature(Type type, boolean useFlatname)
    /// (JNIWriter.java:621-627) through JvmTypeVisitor.
    fn get_jvm_signature(
        &self,
        types: &Types,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        ty: crate::ids::TypeId,
        use_flatname: bool,
    ) -> HeaderResult<String> {
        let t = types.erasure(ctx, syms, ty);
        let mut sig = String::new();
        self.visit_jvm_type(types, ctx, syms, t, use_flatname, &mut sig)?;
        Ok(sig)
    }

    /// Java: class JvmTypeVisitor (JNIWriter.java:569-619).
    fn visit_jvm_type(
        &self,
        types: &Types,
        ctx: &mut CompilerContext,
        syms: &Symtab,
        t: crate::ids::TypeId,
        use_flatname: bool,
        sig: &mut String,
    ) -> HeaderResult<()> {
        match ctx.typ(t).get_tag() {
            TypeTag::VOID => sig.push('V'),
            TypeTag::BOOLEAN => sig.push('Z'),
            TypeTag::BYTE => sig.push('B'),
            TypeTag::CHAR => sig.push('C'),
            TypeTag::SHORT => sig.push('S'),
            TypeTag::INT => sig.push('I'),
            TypeTag::LONG => sig.push('J'),
            TypeTag::FLOAT => sig.push('F'),
            TypeTag::DOUBLE => sig.push('D'),
            TypeTag::ARRAY => {
                sig.push('[');
                let component = match &ctx.typ(t).kind {
                    TypeKindData::ArrayType { elemtype, .. } => {
                        elemtype.expect("array element type")
                    }
                    _ => crate::util::assert::error(),
                };
                let component = types.erasure(ctx, syms, component);
                self.visit_jvm_type(types, ctx, syms, component, use_flatname, sig)?;
            }
            TypeTag::CLASS => {
                let tsym = ctx.typ(t).tsym.expect("AssertionError: class type symbol");
                let classname = if use_flatname {
                    class_flatname(ctx, tsym).to_string_lossy()
                } else {
                    class_qualified_name(ctx, tsym).to_string_lossy()
                };
                sig.push('L');
                sig.push_str(&classname.replace('.', "/"));
                sig.push(';');
            }
            _ => return Err("unknown type: should not happen".to_string()),
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Symbol helpers (the Java methods are ClassSymbol/Symbol members).
// ---------------------------------------------------------------------------

fn has_flag(ctx: &CompilerContext, sym: SymbolId, flag: i64) -> bool {
    (ctx.sym(sym).flags_field & flag) != 0
}

/// Java: Symbol.outermostClass() (Symbol.java:523-531).
fn outermost_class(ctx: &CompilerContext, sym: SymbolId) -> Option<SymbolId> {
    let mut sym = sym;
    let mut prev: Option<SymbolId> = None;
    loop {
        if ctx.sym(sym).kind == Kind::PCK {
            break;
        }
        prev = Some(sym);
        match ctx.sym(sym).owner {
            Some(owner) => sym = owner,
            None => break,
        }
    }
    prev
}

fn class_members(ctx: &mut CompilerContext, class: SymbolId) -> Vec<SymbolId> {
    let scope = match &ctx.sym(class).kind_data {
        SymbolKind::ClassSymbol { members_field, .. } => *members_field,
        _ => None,
    };
    match scope {
        Some(scope) => ctx.scope_get_symbols(
            scope,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
        ),
        None => Vec::new(),
    }
}

fn class_flatname(ctx: &CompilerContext, class: SymbolId) -> JavaString {
    match &ctx.sym(class).kind_data {
        SymbolKind::ClassSymbol { flatname, .. } => flatname.to_java_string(),
        _ => ctx.sym(class).name.to_java_string(),
    }
}

fn class_fullname(ctx: &CompilerContext, class: SymbolId) -> JavaString {
    match &ctx.sym(class).kind_data {
        SymbolKind::ClassSymbol { fullname, .. } => fullname.to_java_string(),
        _ => ctx.sym(class).name.to_java_string(),
    }
}

fn class_qualified_name(ctx: &CompilerContext, class: SymbolId) -> JavaString {
    ctx.sym(class).get_qualified_name().to_java_string()
}

fn module_of_class(ctx: &CompilerContext, class: SymbolId) -> Option<SymbolId> {
    if let Some(owner) = ctx.sym(class).owner
        && ctx.sym(owner).kind == Kind::MDL
    {
        return Some(owner);
    }
    let packge = symbol_packge(ctx, class)?;
    match &ctx.sym(packge).kind_data {
        SymbolKind::PackageSymbol { modle, .. } => *modle,
        _ => None,
    }
}

fn symbol_constant_value(ctx: &CompilerContext, sym: SymbolId) -> Option<ConstantValue> {
    match &ctx.sym(sym).kind_data {
        SymbolKind::VarSymbol { data, .. } => data.clone(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code::source::Source;
    use crate::code::symtab::Symtab;
    use crate::code::types::Types;
    use crate::jvm::target::Target;
    use crate::util::names::Names;
    use std::rc::Rc;

    #[test]
    fn encode_jni_class_and_field_stub_names() {
        let jni = encode_java_string(
            &JavaString::from("java/lang/String"),
            EncoderType::JNI,
        );
        assert_eq!(jni, "java_lang_String");
        let class = encode_java_string(&JavaString::from("a.b$c"), EncoderType::CLASS);
        assert_eq!(class, "a_b__c");
        let stub = encode_java_string(&JavaString::from("a_b"), EncoderType::FIELDSTUB);
        assert_eq!(stub, "a_b");
        let encoded = encode_java_string(&JavaString::from("a(b"), EncoderType::JNI);
        assert_eq!(encoded, "a_00028b");
    }

    #[test]
    fn encode_char_matches_java_hex_escape() {
        assert_eq!(encode_char(0x61, 'a'), "_00061");
        assert_eq!(encode_char(0xe9, 'é'), "_000e9");
    }

    #[test]
    fn jni_primitive_types_match_upstream() {
        let names = Rc::new(Names::new(false));
        let mut ctx = CompilerContext::new();
        let syms = Symtab::new(
            &mut ctx,
            Rc::clone(&names),
            Target::JDK1_17,
            Source::JDK17,
        );
        let types = Types::with_names(Rc::clone(&names));
        for (ty, expected) in [
            (syms.void_type, "void"),
            (syms.boolean_type, "jboolean"),
            (syms.byte_type, "jbyte"),
            (syms.char_type, "jchar"),
            (syms.short_type, "jshort"),
            (syms.int_type, "jint"),
            (syms.long_type, "jlong"),
            (syms.float_type, "jfloat"),
            (syms.double_type, "jdouble"),
        ] {
            assert_eq!(
                jni_type_of(&types, &mut ctx, &syms, ty),
                expected,
                "jni type mapping"
            );
        }
    }
}
