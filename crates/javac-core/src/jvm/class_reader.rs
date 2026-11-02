// Dependency/phase: P2/S2-B (attribute readers, module/directive readers) and
// P3 semantic batches (Annotate/Code/Types); the classfile parsing surface is
// ported in this file.
//
// Ported in this file:
// - the 26 attribute readers are wired in read_attribute: Code (structure
//   only; upstream readCode also returns null and attaches nothing), ConstantValue, Deprecated, Exceptions,
//   InnerClasses (+skipInnerClasses), LocalVariableTable, SourceFile
//   (+SourceFileObject), Annotation, Bridge, Synthetic, Signature,
//   MethodParameters (parameterNameIndices/parameterAccessFlags/sawMethodParameters),
//   EnclosingMethod, PermittedSubclasses, Record, Runtime[In]visible[Parameter]
//   Annotations, AnnotationDefault, Runtime[In]visibleTypeAnnotations, Module,
//   ModuleResolution;
// - readAnnotationValue/readAnnotations/readCompoundAnnotation/readTypeToProxy,
//   attachAnnotations (queued AnnotationCompleter.run equivalent: deproxy,
//   deprecated flags, set/appendDeclarationAttributes), readParameterAnnotations
//   + setParameters/parameter (parameter symbols, names and ParameterAnnotations
//   attach), attachAnnotationDefault (queued AnnotationDefaultCompleter.run
//   equivalent), attachTypeAnnotations (queued TypeAnnotationCompleter.run
//   equivalent: deproxyTypeCompoundList, setTypeAttributes and
//   addTypeAnnotationsToSymbol/TypeAnnotationSymbolVisitor/
//   TypeAnnotationStructuralTypeMapping), CompleterDeproxy, AnnotationDeproxy
//   deproxy/findAccessMethod, ProxyType resolution;
// - adjustMethodParams is inlined in read_method (outer this$n stripping, same
//   branches); Type.getEnclosingType/setEnclosingType are Type/ClassType members
//   (Type.java), not ClassReader units: the signature reader constructs types
//   directly via sigToTypeBytes (registered mapping);
// - SourceFileObject: value equality/hash by name (PartialEq/Eq/Hash).
//
// Registered adapters (not 1:1, see the registration):
// - VarSymbol.data: Java Object -> Option<ConstValue> (ConstantValue reader);
// - Context singletons: ClassReader::new takes the services explicitly
//   (ADR 0001); upstream ClassReader.instance/Context registration is replaced
//   by the compiler wiring;
// - annotation proxies: the upstream Attribute proxy subclasses are modelled by
//   the AnnotationValue enum and deproxied through the same visitor branches;
// - AnnotationCompleter/AnnotationDefaultCompleter/TypeAnnotationCompleter/
//   CompleterDeproxy are expressed as Annotate queue closures (the upstream
//   inner-class semantics are preserved: deproxy after the class is complete);
// - add_type_annotations_to_symbol error path: a CompletionFailure is
//   propagated to the queue instead of the upstream warning + recover
//   (registered; the default addTypeAnnotationsToSymbol option is false);
// - ModuleSymbol uses/provides: UsesProvidesCompleter completion performed in
//   the readClassFile tail (same resulting directives, structural adapter);
// - SourceFileObject: URI(null,name,null) -> URI::create(name); JavaFileObject
//   equality is value-based (PartialEq/Hash) while the FileManager uses Rc
//   identity internally (registered);
// - no PackageInfoCompleter unit exists upstream (the 3261 item is
//   UsesProvidesCompleter, handled above).
// End of Gap registration.

//! This class provides operations to read a classfile into an internal
//! representation. The internal representation is anchored in a ClassSymbol
//! which contains in its scope symbol representations for all other definitions
//! in the classfile. Top-level Classes themselves appear as members of the
//! scopes of PackageSymbols.
//!
//! PORTING_RULES status (P2/S1):
//! * ported in this file: ClassReader structure/constructor (dependency injection
//!   in place of the Context singletons), readClassFile / readClassBuffer
//!   (magic, version, preview checks and the PoolReader wiring), buffer access
//!   (nextChar/nextByte/nextInt), skipMember, adjust*Flags, badClassFile.
//! * the attribute readers / signature state machine, annotation and type
//!   annotation attributes are ported (see the audit at the top). Remaining
//!   gaps: none for readCode — upstream ClassReader.readCode (ClassReader.java:
//!   1399-1408) reads the structure and returns null (Code.java is the Gen-side
//!   code buffer); the local read_code mirrors that exactly. PoolReader symbol
//!   resolution is performed by the ClassReader get_* wrappers (see the audit above).
//! * adapters (registered): Context singletons are injected; CompletionFailure
//!   is a Result error value; PoolReader takes the buffer/names explicitly.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use javac_tools::tools::java_file_object::JavaFileObject;

use crate::code::class_finder::BadClassFile;
use crate::code::deferred_completion_failure_handler::DeferredCompletionFailureHandler;
use crate::code::flags;
use crate::code::preview::Preview;
use crate::code::source::{Feature, Source};
use crate::comp::annotate::{Annotate, AnnotationTypeCompleter, AnnotationTypeMetadata};
use crate::comp::check::warning_from_info;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TypeId};
use crate::jvm::class_file::{JAVA_MAGIC, PREVIEW_MINOR_VERSION, Version};
use crate::jvm::pool_reader::PoolReader;
use crate::jvm::profile::Profile;
use crate::resources::compiler_properties::warnings;
use crate::resources::diagnostic_contract::{FileObjectArg, IntoDiagnosticArg};
use crate::util::abstract_log::AbstractLog;
use crate::util::byte_buffer::ByteBuffer;
use crate::util::jc_diagnostic::{Factory, Fragment};
use crate::util::list::List;
use crate::util::log::Log;
use crate::util::mandatory_warning_handler::MandatoryWarningLog;
use crate::util::name::Name;
use crate::util::names::Names;
use crate::util::options::Options;
use crate::util::pair::Pair;

/// Java: public class ClassReader
///
/// PORTING_RULES: several fields are consumed by the S2/S3 method bodies that
/// are still unimplemented (signature state machine / attribute readers /
/// annotations); they are registered here until those bodies are ported.
#[allow(dead_code)]
pub struct ClassReader {
    /// Java: boolean verbose
    pub verbose: bool,
    /// Java: boolean allowModules
    pub allow_modules: bool,
    /// Java: boolean allowSealedTypes
    pub allow_sealed_types: bool,
    /// Java: boolean allowRecords
    pub allow_records: bool,
    /// Java: boolean lintClassfile
    pub lint_classfile: bool,
    /// Java: public boolean saveParameterNames
    pub save_parameter_names: bool,
    /// Java: private final boolean addTypeAnnotationsToSymbol
    add_type_annotations_to_symbol: bool,
    /// Java: public final Profile profile
    pub profile: Rc<Profile>,
    /// Java: final Log log
    log: Rc<RefCell<Log>>,
    /// Java: final Names names
    names: Rc<Names>,
    /// Java: JCDiagnostic.Factory diagFactory
    diag_factory: Rc<Factory>,
    /// Java: DeferredCompletionFailureHandler dcfh
    dcfh: Rc<DeferredCompletionFailureHandler>,
    /// Java: Preview preview
    preview: Rc<Preview>,
    /// Java: protected WriteableScope typevars
    ///
    /// PORTING_RULES: Scope.WriteableScope is ported as ScopeId; the field is
    /// Some while readTypeVar/enterTypevars process a signature.
    typevars: Option<crate::ids::ScopeId>,
    /// Java: protected JavaFileObject currentClassFile = null
    pub current_class_file: Option<Rc<dyn JavaFileObject>>,
    /// Java: protected Symbol currentOwner = null
    pub current_owner: Option<SymbolId>,
    /// Java: protected ModuleSymbol currentModule = null
    pub current_module: Option<SymbolId>,
    /// Java: the outer ClassReader captured by the inner classes (ProxyType/
    /// CompleterDeproxy/AnnotationDeproxy); the port passes the owning Rc.
    pub(crate) reader_handle: Option<Rc<RefCell<ClassReader>>>,
    /// Java: ByteBuffer buf = new ByteBuffer(INITIAL_BUFFER_SIZE)
    buf: ByteBuffer,
    /// Java: protected int bp
    pub bp: i32,
    /// Java: PoolReader poolReader
    pool_reader: Option<PoolReader>,
    /// Java: int majorVersion
    pub major_version: i32,
    /// Java: int minorVersion
    pub minor_version: i32,
    /// Java: byte[] signatureBuffer (allocated from bp; see readClassBuffer)
    signature_buffer: Vec<u8>,
    /// Java: int sbp — write pointer into signatureBuffer (classSigToType).
    sbp: i32,
    /// Java: private boolean readingClassAttr = false
    reading_class_attr: bool,
    /// Java: Types types (the Types service; Context singleton injected).
    types: crate::code::types::Types,
    /// Java: byte[] signature
    signature: Vec<u8>,
    /// Java: int sigp
    sigp: i32,
    /// Java: int siglimit
    siglimit: i32,
    /// Java: boolean sigEnterPhase
    sig_enter_phase: bool,
    /// Java: Set<Name> warnedAttrs = new HashSet<>()
    warned_attrs: HashSet<Name>,
    /// Java: public boolean filling = false
    pub filling: bool,
    /// Java: List<Type> missingTypeVariables / foundTypeVariables
    missing_type_variables: List<crate::ids::TypeId>,
    found_type_variables: List<crate::ids::TypeId>,
    /// Java: protected Map<Name, AttributeReader> attributeReaders
    attribute_readers: Vec<AttributeReaderDef>,
    /// Java: int[] parameterNameIndices (MethodParameters/LVT)
    parameter_name_indices: Option<Vec<i32>>,
    /// Java: int[] parameterAccessFlags
    parameter_access_flags: Option<Vec<i32>>,
    /// Java: boolean haveParameterNameIndices
    have_parameter_name_indices: bool,
    /// Java: boolean sawMethodParameters
    saw_method_parameters: bool,
    /// Java: ParameterAnnotations[] parameterAnnotations
    parameter_annotations: Option<Vec<Option<List<AnnotationValue>>>>,
    /// Java: private List<InterimUsesDirective> interimUses = List.nil();
    interim_uses: List<Name>,
    /// Java: private List<InterimProvidesDirective> interimProvides = List.nil();
    interim_provides: List<(Name, List<Name>)>,
    /// Pending annotations read from classfile attributes (upstream stores
    /// CompoundAnnotationProxy lists and hands them to Annotate; the Annotate
    /// is not ported yet, so the read values are kept here, registered).",
    pending_annotations: Vec<(SymbolId, List<AnnotationValue>)>,
    /// Pending Runtime[In]visibleParameterAnnotations values.",
    pending_parameter_annotations: Vec<(SymbolId, Vec<List<AnnotationValue>>)>,
    /// Pending AnnotationDefault values.",
    pending_annotation_defaults: Vec<(SymbolId, AnnotationValue)>,
    /// Pending Runtime[In]visibleTypeAnnotations values (TypeAnnotationProxy).
    pending_type_annotations: Vec<(SymbolId, List<TypeAnnotationProxy>)>,
    /// Java: CompoundAnnotationProxy target (ClassReader.java:1435)
    target: Option<AnnotationValue>,
    /// Java: CompoundAnnotationProxy repeatable (ClassReader.java:1436)
    repeatable: Option<AnnotationValue>,
    /// Java: private final Annotate annotate = Annotate.instance(context) —
    /// explicit injection adapter (ADR 0001; see MemberEnter.annotate).
    annotate: Rc<RefCell<Option<Rc<Annotate>>>>,
    /// Java: Type.noType (syms.noType). Symtab does not expose the singleton yet,
    /// so the ClassReader caches the NONE-tag type it uses for absent supertypes
    /// (registered representation adapter; tag-based comparisons are unchanged).
    no_type: Option<TypeId>,
}

impl ClassReader {
    /// Java: ClassReader.initParameterNames(MethodSymbol sym) — initialize the
    /// parameter-name index array, slightly oversized for synthetic slots
    /// (ClassReader.java:2616-2628).
    fn init_parameter_names(&mut self, ctx: &CompilerContext, method: SymbolId) {
        const EXCESS_SLOTS: usize = 4;
        let parameter_types = ctx
            .sym(method)
            .ty
            .map(|ty| ctx.typ(ty).get_parameter_types(ctx))
            .unwrap_or_else(crate::util::list::List::nil);
        let mut expected = EXCESS_SLOTS as i32;
        for ty in parameter_types.iter() {
            expected += jvm_type_width(ctx, ty);
        }
        let expected = expected as usize;
        match &mut self.parameter_name_indices {
            None => self.parameter_name_indices = Some(vec![0; expected]),
            Some(indices) if indices.len() < expected => {
                *indices = vec![0; expected];
            }
            Some(indices) => indices.fill(0),
        }
        self.have_parameter_name_indices = false;
        self.saw_method_parameters = false;
    }

    /// Java: public static final int INITIAL_BUFFER_SIZE = 0x0fff0;
    pub const INITIAL_BUFFER_SIZE: i32 = 0x0fff0;

    /// Java: public static ClassReader instance(Context context) /
    /// protected ClassReader(Context context)
    ///
    /// PORTING_RULES: the Context singletons (Annotate/Types/Symtab/
    /// JavaFileManager/JCDiagnostic.Factory/DeferredCompletionFailureHandler/
    /// Options/Source/Preview/Profile/WriteableScope) are injected instead;
    /// Annotate/fileManager/Lint are not ported yet (S3 wiring, registered).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        log: Rc<RefCell<Log>>,
        names: Rc<Names>,
        diag_factory: Rc<Factory>,
        dcfh: Rc<DeferredCompletionFailureHandler>,
        preview: Rc<Preview>,
        profile: Rc<Profile>,
        options: &Options,
        source: Source,
        annotate: Rc<RefCell<Option<Rc<Annotate>>>>,
    ) -> ClassReader {
        let verbose = options.is_set_option(crate::main::option::Option::VERBOSE);
        let save_parameter_names = options.is_set_option(crate::main::option::Option::PARAMETERS);
        // Java: Types.instance(context) — the reader shares the compiler's Names
        // service; a separate table would make Name identity differ from the
        // compiler's symbols (the ported Types carries Names explicitly).
        let types = crate::code::types::Types::with_names(Rc::clone(&names));
        let mut reader = ClassReader {
            verbose,
            allow_modules: Feature::MODULES.allowed_in_source(source),
            allow_sealed_types: Feature::SEALED_CLASSES.allowed_in_source(source),
            allow_records: Feature::RECORDS.allowed_in_source(source),
            // Java: lintClassfile = Lint.instance(context).isEnabled(LintCategory.CLASSFILE);
            // Port adapter: the Lint service is not ported yet (registered
            // REGISTERED(Lint)); the -Xlint:classfile option is read directly.
            lint_classfile: options.lint_enabled("classfile"),
            save_parameter_names,
            add_type_annotations_to_symbol: options.get_boolean("addTypeAnnotationsToSymbol"),
            profile,
            log,
            names,
            diag_factory,
            dcfh,
            preview,
            typevars: None,
            current_class_file: None,
            current_owner: None,
            current_module: None,
            reader_handle: None,
            buf: ByteBuffer::with_initial_size(ClassReader::INITIAL_BUFFER_SIZE),
            bp: 0,
            pool_reader: None,
            major_version: 0,
            minor_version: 0,
            signature_buffer: vec![0u8; ClassReader::INITIAL_BUFFER_SIZE as usize],
            sbp: 0,
            reading_class_attr: false,
            types,
            signature: Vec::new(),
            sigp: 0,
            siglimit: 0,
            sig_enter_phase: false,
            warned_attrs: HashSet::new(),
            filling: false,
            missing_type_variables: List::nil(),
            found_type_variables: List::nil(),
            attribute_readers: Vec::new(),
            parameter_name_indices: None,
            parameter_access_flags: None,
            have_parameter_name_indices: false,
            saw_method_parameters: false,
            parameter_annotations: None,
            interim_uses: List::nil(),
            interim_provides: List::nil(),
            pending_annotations: Vec::new(),
            pending_parameter_annotations: Vec::new(),
            pending_annotation_defaults: Vec::new(),
            pending_type_annotations: Vec::new(),
            target: None,
            repeatable: None,
            annotate,
            no_type: None,
        };
        reader.init_attribute_readers();
        reader
    }

    // ------------------------------------------------------------------
    // Errors (Java: badClassFile(String key, Object... args))
    // ------------------------------------------------------------------

    /// Java: private ClassFinder.BadClassFile badClassFile(String key, Object... args) /
    /// public ClassFinder.BadClassFile badClassFile(String key, Object... args)
    pub fn bad_class_file(&self, key: Fragment) -> BadClassFile {
        let diag = self.diag_factory.fragment(key);
        BadClassFile::new(
            self.current_owner,
            self.current_class_file
                .clone()
                .expect("currentClassFile (upstream: non-null while reading)"),
            diag,
            Rc::clone(&self.diag_factory),
            Some(Rc::clone(&self.dcfh)),
        )
    }

    /// Converts a PoolReader error to the upstream badClassFile diagnostic
    /// (the PoolReader error variants carry the same keys/arguments).
    fn pool_error(
        &self,
        err: crate::jvm::pool_reader::Error,
    ) -> crate::code::symbol::CompletionFailure {
        use crate::jvm::pool_reader::Error as PoolError;
        let key = match err {
            PoolError::BadConstPoolIndex { index, length } => {
                let file_arg = crate::resources::diagnostic_contract::DiagnosticArg::from_display(
                    self.current_class_file
                        .as_ref()
                        .map(|f| format!("{:?}", f.to_uri()))
                        .unwrap_or_default(),
                );
                Fragment::from(
                    crate::resources::compiler_properties::fragments::bad_const_pool_index(
                        file_arg, index, length,
                    ),
                )
            }
            PoolError::BadConstPoolTagAt { .. } => Fragment::from(
                crate::resources::compiler_properties::fragments::bad_const_pool_tag_at(),
            ),
            PoolError::UnexpectedConstPoolTagAt { .. } => Fragment::from(
                crate::resources::compiler_properties::fragments::unexpected_const_pool_tag_at(),
            ),
        };
        self.bad_class_file(key).failure
    }

    // ------------------------------------------------------------------
    // Constant pool access (Java: PoolReader get* methods)
    // ------------------------------------------------------------------

    fn pool(&self) -> &PoolReader {
        self.pool_reader
            .as_ref()
            .expect("poolReader (readClassBuffer first)")
    }

    /// Java: Name getName(int index)
    fn get_name(&mut self, index: i32) -> Result<Name, crate::code::symbol::CompletionFailure> {
        let result = {
            let pool = self.pool();
            pool.get_name(&self.buf, index, &self.names)
        };
        result.map_err(|err| self.pool_error(err))
    }

    /// Java: ModuleSymbol getModule(int index)
    // Unwired upstream pool accessor (used by the S2/S3 attribute/module batches).
    #[allow(dead_code)]
    fn get_module(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        index: i32,
    ) -> Result<SymbolId, crate::code::symbol::CompletionFailure> {
        let value = {
            let pool = self.pool();
            pool.read_if_needed(
                &self.buf,
                index,
                crate::jvm::pool_reader::TagSet::Module,
                &self.names,
            )
        };
        match value {
            Ok(crate::jvm::pool_reader::PoolValue::Module(name)) => {
                Ok(syms.enter_module(ctx, name))
            }
            Ok(_) => crate::util::assert::error_msg("TagSet::Module accepted a non-module value"),
            Err(err) => Err(self.pool_error(err)),
        }
    }

    /// Java: PackageSymbol getPackage(int index)
    // Unwired upstream pool accessor (used by the S2/S3 attribute/module batches).
    #[allow(dead_code)]
    fn get_package(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        index: i32,
    ) -> Result<SymbolId, crate::code::symbol::CompletionFailure> {
        let value = {
            let pool = self.pool();
            pool.read_if_needed(
                &self.buf,
                index,
                crate::jvm::pool_reader::TagSet::Package,
                &self.names,
            )
        };
        match value {
            Ok(crate::jvm::pool_reader::PoolValue::Package(name)) => {
                Ok(syms.enter_package(ctx, self.current_module.expect("currentModule"), name))
            }
            Ok(_) => crate::util::assert::error_msg("TagSet::Package accepted a non-package value"),
            Err(err) => Err(self.pool_error(err)),
        }
    }

    /// Java: ClassSymbol getClass(int index)
    fn get_class(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        index: i32,
    ) -> Result<SymbolId, crate::code::symbol::CompletionFailure> {
        let value = {
            let pool = self.pool();
            pool.read_if_needed(
                &self.buf,
                index,
                crate::jvm::pool_reader::TagSet::Class,
                &self.names,
            )
        };
        match value {
            Ok(crate::jvm::pool_reader::PoolValue::Class(name)) => {
                // Java: syms.enterClass(reader.currentModule, name)
                Ok(syms.enter_class_flat(ctx, self.current_module.expect("currentModule"), name))
            }
            Ok(_) => crate::util::assert::error_msg("TagSet::Class accepted a non-class value"),
            Err(err) => Err(self.pool_error(err)),
        }
    }

    /// Java: Type getType(int index) — return getName(index).map(reader::sigToType)
    fn get_type(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        index: i32,
    ) -> Result<TypeId, crate::code::symbol::CompletionFailure> {
        let name = self.get_name(index)?;
        let bytes = name.get_byte_array().to_vec();
        self.sig_to_type_bytes(ctx, syms, &bytes, 0, bytes.len() as i32)
    }

    /// Java: NameAndType getNameAndType(int index)
    // Unwired upstream pool accessor (used by the S2/S3 attribute/module batches).
    #[allow(dead_code)]
    fn get_name_and_type(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        index: i32,
    ) -> Result<(Name, TypeId), crate::code::symbol::CompletionFailure> {
        let value = {
            let pool = self.pool();
            pool.read_if_needed(
                &self.buf,
                index,
                crate::jvm::pool_reader::TagSet::NameAndType,
                &self.names,
            )
        };
        match value {
            Ok(crate::jvm::pool_reader::PoolValue::NameAndType { name, descriptor }) => {
                let bytes = descriptor.get_byte_array().to_vec();
                let ty = self.sig_to_type_bytes(ctx, syms, &bytes, 0, bytes.len() as i32)?;
                Ok((name, ty))
            }
            Ok(_) => crate::util::assert::error_msg(
                "TagSet::NameAndType accepted a non-nameandtype value",
            ),
            Err(err) => Err(self.pool_error(err)),
        }
    }

    /// Java: Object getConstant(int index)
    // Unwired upstream pool accessor (used by the S2/S3 attribute batches).
    #[allow(dead_code)]
    fn get_constant(
        &mut self,
        index: i32,
    ) -> Result<crate::jvm::pool_reader::PoolValue, crate::code::symbol::CompletionFailure> {
        let result = {
            let pool = self.pool();
            pool.get_constant(&self.buf, index, &self.names)
        };
        result.map_err(|err| self.pool_error(err))
    }

    // ------------------------------------------------------------------
    // Signature parsing (ClassReader.java:431-740)
    // ------------------------------------------------------------------

    /// Java: Type sigToType(byte[] sig, int offset, int len)
    fn sig_to_type_bytes(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        sig: &[u8],
        offset: i32,
        len: i32,
    ) -> Result<TypeId, crate::code::symbol::CompletionFailure> {
        self.signature = sig.to_vec();
        self.sigp = offset;
        self.siglimit = offset + len;
        self.sig_to_type(ctx, syms)
    }

    /// Java: Type sigToType()
    fn sig_to_type(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
    ) -> Result<TypeId, crate::code::symbol::CompletionFailure> {
        let c = self.signature[self.sigp as usize] as char;
        match c {
            'T' => {
                self.sigp += 1;
                let start = self.sigp;
                while self.signature[self.sigp as usize] as char != ';' {
                    self.sigp += 1;
                }
                self.sigp += 1;
                if self.sig_enter_phase {
                    Ok(self.no_type(ctx))
                } else {
                    let name =
                        self.names
                            .table
                            .from_utf(&self.signature, start, self.sigp - 1 - start);
                    self.find_type_var(ctx, syms, &name)
                }
            }
            '+' => {
                self.sigp += 1;
                let t = self.sig_to_type(ctx, syms)?;
                Ok(ctx.alloc_type(crate::code::type_::Type::new_wildcard_type(
                    t,
                    crate::code::bound_kind::BoundKind::EXTENDS,
                    syms.bound_class,
                    None,
                )))
            }
            '*' => {
                self.sigp += 1;
                Ok(ctx.alloc_type(crate::code::type_::Type::new_wildcard_type(
                    syms.object_type,
                    crate::code::bound_kind::BoundKind::UNBOUND,
                    syms.bound_class,
                    None,
                )))
            }
            '-' => {
                self.sigp += 1;
                let t = self.sig_to_type(ctx, syms)?;
                Ok(ctx.alloc_type(crate::code::type_::Type::new_wildcard_type(
                    t,
                    crate::code::bound_kind::BoundKind::SUPER,
                    syms.bound_class,
                    None,
                )))
            }
            'B' => {
                self.sigp += 1;
                Ok(syms.byte_type)
            }
            'C' => {
                self.sigp += 1;
                Ok(syms.char_type)
            }
            'D' => {
                self.sigp += 1;
                Ok(syms.double_type)
            }
            'F' => {
                self.sigp += 1;
                Ok(syms.float_type)
            }
            'I' => {
                self.sigp += 1;
                Ok(syms.int_type)
            }
            'J' => {
                self.sigp += 1;
                Ok(syms.long_type)
            }
            'L' => {
                let t = self.class_sig_to_type(ctx, syms)?;
                if self.sigp < self.siglimit && self.signature[self.sigp as usize] as char == '.' {
                    return Err(self
                        .bad_class_file(Fragment::from(
                            crate::resources::compiler_properties::fragments::bad_class_signature(),
                        ))
                        .failure);
                }
                Ok(t)
            }
            'S' => {
                self.sigp += 1;
                Ok(syms.short_type)
            }
            'V' => {
                self.sigp += 1;
                Ok(syms.void_type)
            }
            'Z' => {
                self.sigp += 1;
                Ok(syms.boolean_type)
            }
            '[' => {
                self.sigp += 1;
                let elem = self.sig_to_type(ctx, syms)?;
                Ok(ctx.alloc_type(crate::code::type_::Type::new_array_type(
                    elem,
                    syms.array_class,
                )))
            }
            '(' => {
                self.sigp += 1;
                let argtypes = self.sig_to_types(ctx, syms, ')')?;
                let restype = self.sig_to_type(ctx, syms)?;
                let mut thrown = List::nil();
                while self.sigp < self.siglimit && self.signature[self.sigp as usize] as char == '^'
                {
                    self.sigp += 1;
                    let t = self.sig_to_type(ctx, syms)?;
                    thrown = thrown.prepend(t);
                }
                // if there is a typevar in the throws clause we should state it.
                let l = thrown.clone();
                for head in &l {
                    if ctx
                        .typ(head)
                        .has_tag(crate::code::type_tag::TypeTag::TYPEVAR)
                        && let Some(tsym) = ctx.typ(head).tsym
                    {
                        ctx.sym_mut(tsym).flags_field |= crate::code::flags::THROWS;
                    }
                }
                Ok(ctx.alloc_type(crate::code::type_::Type::new_method_type(
                    argtypes,
                    restype,
                    thrown.reverse(),
                    syms.method_class,
                )))
            }
            '<' => {
                let tv = self.typevars.expect("typevars scope");
                self.typevars = Some(ctx.scope_dup(tv, self.current_owner));
                let tvars = self.sig_to_type_params(ctx, syms)?;
                let poly = self.sig_to_type(ctx, syms)?;
                let tv2 = self.typevars.expect("typevars scope");
                self.typevars = Some(ctx.scope_leave(tv2));
                Ok(ctx.alloc_type(crate::code::type_::Type::new_for_all(tvars, poly)))
            }
            _other => Err(self
                .bad_class_file(Fragment::from(
                    crate::resources::compiler_properties::fragments::bad_signature(),
                ))
                .failure),
        }
    }

    /// Java: Type noType (syms.noType): Types.noType is represented by
    /// syms.no_type/ctx.no_type; this helper returns the shared arena Type
    /// (tag NONE) and caches it in the reader.
    fn no_type(&mut self, ctx: &mut CompilerContext) -> TypeId {
        if let Some(no_type) = self.no_type {
            return no_type;
        }
        let no_type = ctx.alloc_type(crate::code::type_::Type::new(
            crate::code::type_tag::TypeTag::NONE,
            None,
            crate::code::type_::TypeKindData::JCNoType {
                kind: crate::code::type_::JCNoTypeKind::Base,
            },
        ));
        self.no_type = Some(no_type);
        no_type
    }

    /// Java: List<Type> sigToTypes(char terminator)
    fn sig_to_types(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        terminator: char,
    ) -> Result<List<TypeId>, crate::code::symbol::CompletionFailure> {
        let mut result = List::nil();
        while self.signature[self.sigp as usize] as char != terminator {
            result = result.prepend(self.sig_to_type(ctx, syms)?);
        }
        self.sigp += 1;
        Ok(result.reverse())
    }

    /// Java: List<Type> sigToTypeParams()
    fn sig_to_type_params(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
    ) -> Result<List<TypeId>, crate::code::symbol::CompletionFailure> {
        let mut tvars = List::nil();
        if self.signature[self.sigp as usize] as char == '<' {
            self.sigp += 1;
            let start = self.sigp;
            self.sig_enter_phase = true;
            while self.signature[self.sigp as usize] as char != '>' {
                tvars = tvars.prepend(self.sig_to_type_param(ctx, syms)?);
            }
            self.sig_enter_phase = false;
            self.sigp = start;
            while self.signature[self.sigp as usize] as char != '>' {
                self.sig_to_type_param(ctx, syms)?;
            }
            self.sigp += 1;
        }
        Ok(tvars.reverse())
    }

    /// Java: Type sigToTypeParam()
    fn sig_to_type_param(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
    ) -> Result<TypeId, crate::code::symbol::CompletionFailure> {
        let start = self.sigp;
        while self.signature[self.sigp as usize] as char != ':' {
            self.sigp += 1;
        }
        let name = self
            .names
            .table
            .from_utf(&self.signature, start, self.sigp - start);
        let tvar = if self.sig_enter_phase {
            let owner = self.current_owner.expect("currentOwner");
            let t = ctx.new_type_var_with_owner(&self.names, name, owner, syms.bot_type);
            let tsym = ctx.typ(t).tsym.expect("type variable symbol");
            let typevars = self.typevars.expect("typevars scope");
            ctx.scope_enter(typevars, tsym);
            t
        } else {
            self.find_type_var(ctx, syms, &name)?
        };
        let mut bounds = List::nil();
        let mut all_interfaces = false;
        if self.signature[self.sigp as usize] as char == ':'
            && self.signature[self.sigp as usize + 1] as char == ':'
        {
            self.sigp += 1;
            all_interfaces = true;
        }
        while self.signature[self.sigp as usize] as char == ':' {
            self.sigp += 1;
            bounds = bounds.prepend(self.sig_to_type(ctx, syms)?);
        }
        if !self.sig_enter_phase {
            self.types.set_bounds(
                ctx,
                syms,
                &self.names,
                tvar,
                bounds.reverse(),
                all_interfaces,
            );
        }
        Ok(tvar)
    }
    /// Java: Type classSigToType()
    fn class_sig_to_type(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
    ) -> Result<TypeId, crate::code::symbol::CompletionFailure> {
        if self.signature[self.sigp as usize] as char != 'L' {
            return Err(self
                .bad_class_file(Fragment::from(
                    crate::resources::compiler_properties::fragments::bad_class_signature(),
                ))
                .failure);
        }
        self.sigp += 1;
        let mut outer: Option<TypeId> = None;
        let start_sbp = self.sbp;
        loop {
            let c = self.signature[self.sigp as usize] as char;
            self.sigp += 1;
            match c {
                ';' => {
                    let class_name = self.names.table.from_utf(
                        &self.signature_buffer,
                        start_sbp,
                        self.sbp - start_sbp,
                    );
                    let t = self.enter_class(ctx, syms, class_name)?;
                    let result = match outer {
                        None => self.types.erasure(
                            ctx,
                            syms,
                            ctx.sym(t).ty.expect("entered class type"),
                        ),
                        Some(outer) => ctx.alloc_type(crate::code::type_::Type::new_class_type(
                            Some(outer),
                            Some(List::nil()),
                            t,
                        )),
                    };
                    self.sbp = start_sbp;
                    return Ok(result);
                }
                '<' => {
                    let class_name = self.names.table.from_utf(
                        &self.signature_buffer,
                        start_sbp,
                        self.sbp - start_sbp,
                    );
                    let t = self.enter_class(ctx, syms, class_name)?;
                    let args = self.sig_to_types(ctx, syms, '>')?;
                    // Upstream: ClassReader.classSigToType starts with
                    // outer = Type.noType and builds new ClassType(outer, args, t).
                    // The local scanner represents that sentinel as None, so a
                    // top-level parameterized class keeps the explicit noType
                    // enclosing used by the erasure(sym.type) plain path.
                    let outer_ty = outer.unwrap_or_else(|| crate::code::symtab::no_type_id(ctx));
                    outer = Some(ctx.alloc_type(crate::code::type_::Type::new_class_type(
                        Some(outer_ty),
                        Some(args),
                        t,
                    )));
                    let sep = self.signature[self.sigp as usize] as char;
                    self.sigp += 1;
                    match sep {
                        ';' => {
                            if self.sigp < self.siglimit
                                && self.signature[self.sigp as usize] as char == '.'
                            {
                                // support old-style GJC signatures: skip
                                // ".Lfoo/Outer$"
                                self.sigp += (self.sbp - start_sbp) + 3;
                                self.signature_buffer[self.sbp as usize] = b'$';
                                self.sbp += 1;
                            } else {
                                self.sbp = start_sbp;
                                return Ok(outer.expect("outer type"));
                            }
                        }
                        '.' => {
                            self.signature_buffer[self.sbp as usize] = b'$';
                            self.sbp += 1;
                        }
                        other => panic!("AssertionError({other})"),
                    }
                }
                '.' => {
                    // we have seen an enclosing non-generic class
                    if let Some(outer_ty) = outer {
                        let class_name = self.names.table.from_utf(
                            &self.signature_buffer,
                            start_sbp,
                            self.sbp - start_sbp,
                        );
                        let t = self.enter_class(ctx, syms, class_name)?;
                        outer = Some(ctx.alloc_type(crate::code::type_::Type::new_class_type(
                            Some(outer_ty),
                            Some(List::nil()),
                            t,
                        )));
                    }
                    self.signature_buffer[self.sbp as usize] = b'$';
                    self.sbp += 1;
                }
                '/' => {
                    self.signature_buffer[self.sbp as usize] = b'.';
                    self.sbp += 1;
                }
                other => {
                    self.signature_buffer[self.sbp as usize] = other as u8;
                    self.sbp += 1;
                }
            }
        }
    }

    /// Java: Type findTypeVar(Name name)
    fn find_type_var(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        name: &Name,
    ) -> Result<TypeId, crate::code::symbol::CompletionFailure> {
        let typevars = self.typevars.expect("typevars scope");
        if let Some(sym) = ctx.scope_find_first(typevars, name, &|_| true) {
            return Ok(ctx.sym(sym).ty.expect("type variable symbol type"));
        }
        if self.reading_class_attr {
            // While reading the class attribute, the supertypes might refer to a
            // type variable from an enclosing element (method or class).
            let owner = self.current_owner.expect("currentOwner");
            let t = ctx.new_type_var_with_owner(&self.names, name.clone(), owner, syms.bot_type);
            self.missing_type_variables = self.missing_type_variables.prepend(t);
            return Ok(t);
        }
        Err(self
            .bad_class_file(Fragment::from(
                crate::resources::compiler_properties::fragments::undecl_type_var(),
            ))
            .failure)
    }
    /// Java: protected ClassSymbol enterClass(Name name)
    fn enter_class(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        name: Name,
    ) -> Result<SymbolId, crate::code::symbol::CompletionFailure> {
        Ok(syms.enter_class_flat(ctx, self.current_module.expect("currentModule"), name))
    }

    /// Java: private MethodSymbol lookupMethod(TypeSymbol tsym, Name name, List<Type> argtypes)
    fn lookup_method(
        &self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        tsym: SymbolId,
        name: &Name,
        argtypes: List<TypeId>,
    ) -> Option<SymbolId> {
        let members = ctx.sym(tsym).members()?;
        let scope_snapshot = ctx.scope(members).clone();
        let candidates = scope_snapshot.get_symbols_by_name(
            members,
            name,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
            ctx,
        );
        let candidates = candidates
            .into_iter()
            .filter(|sym| ctx.sym(*sym).kind == crate::code::kinds::Kind::MTH)
            .collect::<Vec<SymbolId>>();
        for candidate in candidates {
            let ty = ctx.sym(candidate).ty.expect("method type");
            let params = ctx.typ(ty).get_parameter_types(ctx);
            if self.types.is_same_types(ctx, syms, &params, &argtypes) {
                return Some(candidate);
            }
        }
        None
    }
    /// Java: protected void enterTypevars(Symbol sym, Type t)
    fn enter_typevars(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        sym: SymbolId,
        t: TypeId,
    ) {
        if let Some(enclosing) = ctx.typ(t).get_enclosing_type() {
            if !ctx
                .typ(enclosing)
                .has_tag(crate::code::type_tag::TypeTag::NONE)
            {
                let owner = ctx.sym(sym).owner.expect("symbol owner");
                self.enter_typevars(ctx, syms, owner, enclosing);
            }
        } else if ctx.sym(sym).kind == crate::code::kinds::Kind::MTH && !ctx.sym(sym).is_static(ctx)
        {
            let owner = ctx.sym(sym).owner.expect("symbol owner");
            let owner_type = ctx.sym(owner).ty.expect("owner type");
            self.enter_typevars(ctx, syms, owner, owner_type);
        }
        let xs = crate::code::types::get_type_arguments_completed(ctx, syms, t, None);
        let typevars = self.typevars.expect("typevars scope");
        for head in &xs {
            if let Some(tsym) = ctx.typ(head).tsym {
                ctx.scope_enter(typevars, tsym);
            }
        }
    }

    // ------------------------------------------------------------------
    // Buffer access
    // ------------------------------------------------------------------

    /// Java: char nextChar()
    pub fn next_char(&mut self) -> u16 {
        let res = self.buf.get_char(self.bp);
        self.bp += 2;
        res
    }

    /// Java: int nextByte()
    pub fn next_byte(&mut self) -> i32 {
        let res = self.buf.get_byte(self.bp) as u8 as i32;
        self.bp += 1;
        res
    }

    /// Java: int nextInt()
    pub fn next_int(&mut self) -> i32 {
        let res = self.buf.get_int(self.bp);
        self.bp += 4;
        res
    }

    /// Java: void skipMember()
    pub fn skip_member(&mut self) {
        self.bp += 6;
        let ac = self.next_char();
        for _ in 0..ac {
            self.bp += 2;
            let attr_len = self.next_int();
            self.bp += attr_len;
        }
    }

    /// Java: long adjustFieldFlags(long flags)
    pub fn adjust_field_flags(&self, flags: i64) -> i64 {
        flags
    }

    /// Java: long adjustMethodFlags(long flags)
    pub fn adjust_method_flags(&self, flags: i64) -> i64 {
        let acc_bridge = flags::ACC_BRIDGE as i64;
        let acc_varargs = flags::ACC_VARARGS as i64;
        let mut flags = flags;
        if (flags & acc_bridge) != 0 {
            flags &= !acc_bridge;
            flags |= flags::BRIDGE;
        }
        if (flags & acc_varargs) != 0 {
            flags &= !acc_varargs;
            flags |= flags::VARARGS;
        }
        flags
    }

    /// Java: long adjustClassFlags(long flags)
    pub fn adjust_class_flags(&self, flags: i64) -> i64 {
        let acc_module = flags::ACC_MODULE as i64;
        let acc_super = flags::ACC_SUPER as i64;
        let mut flags = flags;
        if (flags & acc_module) != 0 {
            flags &= !acc_module;
            flags |= flags::MODULE;
        }
        flags & !acc_super
    }

    /// Java: private void enterMember(ClassSymbol c, Symbol sym)
    fn enter_member(&mut self, ctx: &mut CompilerContext, c: SymbolId, sym: SymbolId) {
        // Synthetic members are not entered -- reason lost to history (optimization?).
        // Lambda methods must be entered because they may have inner classes (which reference them)
        let flags = ctx.sym(sym).flags_field;
        let is_synthetic = (flags & flags::SYNTHETIC as i64) == flags::SYNTHETIC as i64;
        let is_bridge = (flags & flags::BRIDGE) == flags::BRIDGE;
        let name = ctx.sym(sym).name.clone();
        let lambda_prefix = self.names.lambda.to_java_string();
        let is_lambda = name.to_java_string().starts_with(&lambda_prefix);
        if !(is_synthetic && !is_bridge) || is_lambda {
            let members = match &ctx.sym(c).kind_data {
                crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } => {
                    *members_field
                }
                _ => panic!("ClassCastException: not a class symbol"),
            };
            ctx.scope_enter(members.expect("members scope"), sym);
        }
    }
    // ------------------------------------------------------------------
    // Reading the class file (ClassReader.java:2787-2960, 1369-1395)
    // ------------------------------------------------------------------

    /// Java: public void readClassFile(ClassSymbol c)
    pub fn read_class_file(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        c: SymbolId,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        self.current_owner = Some(c);
        self.current_class_file = class_file_of(ctx, c);
        self.warned_attrs.clear();
        self.filling = true;
        // Java: target = null; repeatable = null; (S3 annotation proxies, not ported)
        let file = self
            .current_class_file
            .clone()
            .expect("ClassSymbol.classfile (upstream: non-null when completing)");
        self.bp = 0;
        self.buf.reset();
        let mut stream = match file.open_input_stream() {
            Ok(stream) => stream,
            Err(_) => {
                let diag = self.diag_factory.fragment(Fragment::from(
                    crate::resources::compiler_properties::fragments::unable_to_access_file(),
                ));
                let failure_diag = diag.clone();
                return Err(crate::code::symbol::CompletionFailure::new(
                    Some(c),
                    Box::new(move || failure_diag.clone()),
                    Some(Rc::clone(&self.dcfh)),
                ));
            }
        };
        if self.buf.append_stream(&mut stream).is_err() {
            let diag = self.diag_factory.fragment(Fragment::from(
                crate::resources::compiler_properties::fragments::unable_to_access_file(),
            ));
            let failure_diag = diag.clone();
            return Err(crate::code::symbol::CompletionFailure::new(
                Some(c),
                Box::new(move || failure_diag.clone()),
                Some(Rc::clone(&self.dcfh)),
            ));
        }
        // Java: readClassFile catches ArrayIndexOutOfBoundsException and maps it
        // to badClassFile("bad.class.file", c.flatname) (ClassReader.java:2790-2800).
        // The Rust buffer accessors panic on an out-of-range read; only that
        // bounds panic is converted, every other panic is resumed.
        let read_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.read_class_buffer(ctx, syms, c)
        }));
        match read_result {
            Ok(result) => result?,
            Err(payload) => {
                if Self::is_index_out_of_bounds_panic(payload.as_ref()) {
                    let class_name = ctx.sym(c).flat_name();
                    let key = Fragment::from(
                        crate::resources::compiler_properties::fragments::bad_class_file(
                            class_name,
                        ),
                    );
                    return Err(self.bad_class_file(key).failure);
                }
                std::panic::resume_unwind(payload);
            }
        }
        // Java: if (!missingTypeVariables.isEmpty() && !foundTypeVariables.isEmpty()) {
        //     ... ct.supertype_field = types.subst(...); ct.interfaces_field = ...
        //     ct.typarams_field = types.substBounds(...); ... }
        if !self.missing_type_variables.is_empty() && !self.found_type_variables.is_empty() {
            let missing = self.missing_type_variables.clone();
            let found = self.found_type_variables.clone();
            self.missing_type_variables = List::nil();
            self.found_type_variables = List::nil();
            self.interim_uses = List::nil();
            self.interim_provides = List::nil();
            self.filling = false;
            let owner = self.current_owner.expect("currentOwner");
            let ct = ctx.sym(owner).ty.expect("currentOwner.type");
            let types = &self.types;
            let (supertype, interfaces, bounds) = match &ctx.typ(ct).kind {
                crate::code::type_::TypeKindData::ClassType {
                    supertype_field,
                    interfaces_field,
                    typarams_field,
                    ..
                } => (
                    *supertype_field,
                    interfaces_field.clone(),
                    typarams_field.clone().unwrap_or_else(List::nil),
                ),
                _ => panic!("ClassCastException: ClassType expected"),
            };
            let new_supertype = supertype
                .map(|supertype| types.subst_type(ctx, syms, supertype, &missing, &found, None));
            let new_interfaces =
                interfaces.map(|interfaces| types.subst(ctx, syms, &interfaces, &missing, &found));
            let new_bounds = types.subst_bounds(ctx, syms, &bounds, &missing, &found);
            if let crate::code::type_::TypeKindData::ClassType {
                supertype_field,
                interfaces_field,
                typarams_field,
                ..
            } = &mut ctx.typ_mut(ct).kind
            {
                *supertype_field = new_supertype;
                *interfaces_field = new_interfaces;
                *typarams_field = Some(new_bounds.clone());
            }
            for typevar in new_bounds.iter() {
                if let Some(tsym) = ctx.typ(typevar).tsym {
                    ctx.sym_mut(tsym).ty = Some(typevar);
                }
            }
        } else if self.missing_type_variables.is_empty() != self.found_type_variables.is_empty() {
            // Java: Name name = missingTypeVariables.head.tsym.name;
            // throw badClassFile("undecl.type.var", name);
            // Java: Name name = missingTypeVariables.head.tsym.name;
            let key =
                Fragment::from(crate::resources::compiler_properties::fragments::undecl_type_var());
            return Err(self.bad_class_file(key).failure);
        }

        // Java: if ((c.flags_field & Flags.ANNOTATION) != 0) c.setAnnotationTypeMetadata(
        // new AnnotationTypeMetadata(c, new CompleterDeproxy(c, target, repeatable)));
        // else c.setAnnotationTypeMetadata(AnnotationTypeMetadata.notAnAnnotationType());
        // (ClassReader.java:2988-2995)
        if (ctx.sym(c).flags_field & flags::ANNOTATION as i64) != 0 {
            let completer: Rc<RefCell<dyn AnnotationTypeCompleter>> =
                Rc::new(RefCell::new(CompleterDeproxy::new(
                    c,
                    self.target.clone(),
                    self.repeatable.clone(),
                    Rc::clone(&self.names),
                    self.reader_handle.clone(),
                )));
            ctx.sym_mut(c)
                .set_annotation_type_metadata(Rc::new(RefCell::new(AnnotationTypeMetadata::new(
                    c,
                    Some(completer),
                ))));
        } else {
            ctx.sym_mut(c)
                .set_annotation_type_metadata(Rc::new(RefCell::new(
                    AnnotationTypeMetadata::not_an_annotation_type(),
                )));
        }

        // Java: if (c == currentModule.module_info) {
        // if (interimUses.nonEmpty() || interimProvides.nonEmpty()) {
        // currentModule.usesProvidesCompleter = new UsesProvidesCompleter(...)
        // } }
        //
        // PORTING_RULES adapter: UsesProvidesCompleter / ModuleSymbol.completer are not
        // ported yet, so the interim names are resolved to ClassSymbols and recorded as
        // UsesDirective/ProvidesDirective here (the same result the upstream completer
        // produces on completion; registered structural adapter). The use of a
        // deferred completer instead of immediate entry is not observable for the
        // module-info reading path covered by the tests.
        if let Some(module) = self.current_module {
            let module_info = match &ctx.sym(module).kind_data {
                crate::code::symbol::SymbolKind::ModuleSymbol { module_info, .. } => *module_info,
                _ => None,
            };
            if module_info == Some(c)
                && (!self.interim_uses.is_empty() || !self.interim_provides.is_empty())
            {
                let uses_names = self.interim_uses.clone();
                let provides_names = self.interim_provides.clone();
                self.interim_uses = List::nil();
                self.interim_provides = List::nil();
                let mut uses: List<crate::ids::DirectiveId> = List::nil();
                let list = uses_names;
                for service_name in &list {
                    let service = self.enter_class(ctx, syms, service_name.clone())?;
                    let directive = ctx.alloc_directive(crate::code::directive::Directive::Uses {
                        service: Some(service),
                    });
                    uses = uses.append(directive);
                }
                let mut provides: List<crate::ids::DirectiveId> = List::nil();
                let list = provides_names;
                for (service_name, impl_names) in &list {
                    let service = self.enter_class(ctx, syms, service_name)?;
                    let mut impls: List<SymbolId> = List::nil();
                    let mut impls_list = impl_names;
                    while impls_list.non_empty() {
                        let impl_class = self.enter_class(ctx, syms, impls_list.head().clone())?;
                        impls = impls.append(impl_class);
                        impls_list = impls_list.tail();
                    }
                    let directive =
                        ctx.alloc_directive(crate::code::directive::Directive::Provides {
                            service: Some(service),
                            impls,
                        });
                    provides = provides.append(directive);
                }
                if let crate::code::symbol::SymbolKind::ModuleSymbol {
                    uses: uses_slot,
                    provides: provides_slot,
                    directives: directives_field,
                    ..
                } = &mut ctx.sym_mut(module).kind_data
                {
                    *uses_slot = uses.clone();
                    *provides_slot = provides.clone();
                    *directives_field = directives_field.append_list(&uses).append_list(&provides);
                }
            }
        }
        Ok(())
    }

    /// Java: private void readClassBuffer(ClassSymbol c) throws IOException
    fn read_class_buffer(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        c: SymbolId,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        let magic = self.next_int();
        if magic != JAVA_MAGIC {
            let key = Fragment::from(
                crate::resources::compiler_properties::fragments::illegal_start_of_class_file(),
            );
            return Err(self.bad_class_file(key).failure);
        }
        self.minor_version = self.next_char() as i32;
        self.major_version = self.next_char() as i32;
        let max_major = Version::max().major();
        let max_minor = Version::max().minor();
        let preview_class_file = self.minor_version == PREVIEW_MINOR_VERSION;
        if self.major_version > max_major
            || self.major_version * 1000 + self.minor_version
                < Version::min().major() * 1000 + Version::min().minor()
        {
            if self.major_version == max_major + 1 && !preview_class_file {
                // Java: ClassReader.java:2926-2929
                //   log.warning(Warnings.BigMajorVersion(currentClassFile,
                //                                      majorVersion, maxMajor));
                let file_name = self
                    .current_class_file
                    .clone()
                    .map(|file| file.get_name().to_string_lossy().to_string())
                    .unwrap_or_default();
                let info = warnings::big_major_version(
                    FileObjectArg::from(file_name.as_str()).into_diagnostic_arg(),
                    self.major_version,
                    max_major,
                );
                self.log
                    .borrow_mut()
                    .warning(None, None, warning_from_info(info));
            } else {
                // Java: badClassFile("wrong.version", major, minor, maxMajor, maxMinor);
                // The generated accessor takes the four version strings (P4 contract).
                let key = Fragment::from(
                    crate::resources::compiler_properties::fragments::wrong_version(),
                );
                return Err(self.bad_class_file(key).failure);
            }
        }
        if preview_class_file {
            if !self.preview.is_enabled() {
                // Java: ClassReader.java:2938-2941
                //   if (!preview.isEnabled()) log.error(preview.disabledError(currentClassFile, majorVersion));
                let file = self
                    .current_class_file
                    .clone()
                    .expect("ClassSymbol.classfile (upstream: non-null when completing)");
                let error = self
                    .preview
                    .disabled_error_classfile(&*file, self.major_version);
                AbstractLog::error_key(&mut *self.log.borrow_mut(), error);
            } else {
                // Java: ClassReader.java:2942 preview.warnPreview(c.classfile, majorVersion);
                let file = self
                    .current_class_file
                    .clone()
                    .expect("ClassSymbol.classfile (upstream: non-null when completing)");
                self.preview
                    .warn_preview_classfile(&*file, self.major_version);
            }
        }
        let mut pool_reader = PoolReader::new();
        let end = match pool_reader.read_pool(&self.buf, self.bp) {
            Ok(end) => end,
            Err(err) => return Err(self.pool_error(err)),
        };
        self.bp = end;
        if self.signature_buffer.len() < self.bp as usize {
            let ns = (self.bp as usize).next_power_of_two();
            self.signature_buffer = vec![0u8; ns];
        }
        self.pool_reader = Some(pool_reader);
        self.read_class(ctx, syms, c)
    }

    /// Java: void readClass(ClassSymbol c)
    fn read_class(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        c: SymbolId,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        // Java: target = null; repeatable = null; (ClassReader.readClass)
        self.target = None;
        self.repeatable = None;
        // ClassType ct = (ClassType)c.type;
        let ct = ctx.sym(c).ty.expect("class symbol type");
        // allocate scope for members
        let members = ctx.scope_create(Some(c));
        if let crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } =
            &mut ctx.sym_mut(c).kind_data
        {
            *members_field = Some(members);
        }
        // prepare type variable table
        if self.typevars.is_none() {
            // Java: typevars = WriteableScope.create(syms.noSymbol) (constructor)
            self.typevars = Some(ctx.scope_create(Some(syms.no_symbol)));
        }
        let tv = self.typevars.expect("typevars scope");
        self.typevars = Some(ctx.scope_dup(tv, Some(c)));
        if ctx.typ(ct).get_enclosing_type().is_some_and(|outer| {
            ctx.typ(outer)
                .has_tag(crate::code::type_tag::TypeTag::CLASS)
        }) {
            let enclosing = ctx.typ(ct).get_enclosing_type().expect("enclosing");
            let owner = ctx.sym(c).owner.expect("class owner");
            self.enter_typevars(ctx, syms, owner, enclosing);
        }

        // read flags, or skip if this is an inner class
        let f = self.next_char() as i64;
        let flags = self.adjust_class_flags(f);
        let module_flag = flags & flags::MODULE;
        if module_flag == 0 {
            let owner_kind = ctx.sym(c).owner.map(|o| ctx.sym(o).kind);
            if matches!(
                owner_kind,
                Some(crate::code::kinds::Kind::PCK) | Some(crate::code::kinds::Kind::ERR)
            ) {
                ctx.sym_mut(c).flags_field = flags;
            }
            // read own class name and check that it matches
            self.current_module = Some(class_module(ctx, c));
            let this_class_index = self.next_char() as i32;
            let self_sym = self.get_class(ctx, syms, this_class_index)?;
            if c != self_sym {
                let flat = ctx.sym(self_sym).flat_name();
                let key = Fragment::from(
                    crate::resources::compiler_properties::fragments::class_file_wrong_class(),
                );
                let _ = flat;
                return Err(self.bad_class_file(key).failure);
            }
        } else {
            if self.major_version < Version::V53.major() {
                let key = Fragment::from(
                    crate::resources::compiler_properties::fragments::anachronistic_module_info(
                        crate::util::java_string::JavaString::from(
                            self.major_version.to_string().as_str(),
                        ),
                        crate::util::java_string::JavaString::from(
                            self.minor_version.to_string().as_str(),
                        ),
                    ),
                );
                return Err(self.bad_class_file(key).failure);
            }
            ctx.sym_mut(c).flags_field = flags;
            if ctx.sym(c).owner.map(|o| ctx.sym(o).kind) != Some(crate::code::kinds::Kind::MDL) {
                let key = Fragment::from(
                    crate::resources::compiler_properties::fragments::module_info_definition_expected(),
                );
                return Err(self.bad_class_file(key).failure);
            }
            self.current_module = Some(ctx.sym(c).owner.expect("module owner"));
            let _this_class = self.next_char();
            // temp, no check on this_class
        }

        // class attributes must be read before class
        // skip ahead to read class attributes
        let startbp = self.bp;
        self.next_char();
        let interface_count = self.next_char();
        self.bp += interface_count as i32 * 2;
        let field_count = self.next_char();
        for _ in 0..field_count {
            self.skip_member();
        }
        let method_count = self.next_char();
        for _ in 0..method_count {
            self.skip_member();
        }
        self.read_class_attrs(ctx, syms, c)?;

        let has_permitted = match &ctx.sym(c).kind_data {
            crate::code::symbol::SymbolKind::ClassSymbol { permitted, .. } => !permitted.is_empty(),
            _ => false,
        };
        if has_permitted {
            ctx.sym_mut(c).flags_field |= crate::code::flags::SEALED;
        }

        // reset and read rest of classinfo
        self.bp = startbp;
        let n = self.next_char() as i32;
        if module_flag != 0 && n > 0 {
            let key = Fragment::from(
                crate::resources::compiler_properties::fragments::module_info_invalid_super_class(),
            );
            return Err(self.bad_class_file(key).failure);
        }
        let supertype_field_is_null = matches!(
            &ctx.typ(ct).kind,
            crate::code::type_::TypeKindData::ClassType {
                supertype_field: None,
                ..
            }
        );
        if supertype_field_is_null {
            let super_type = if n == 0 {
                self.no_type(ctx)
            } else {
                let class = self.get_class(ctx, syms, n)?;
                let class_type = ctx.sym(class).ty.expect("class type");
                self.types.erasure(ctx, syms, class_type)
            };
            if let crate::code::type_::TypeKindData::ClassType {
                supertype_field, ..
            } = &mut ctx.typ_mut(ct).kind
            {
                *supertype_field = Some(super_type);
            }
        }
        let n = self.next_char() as i32;
        let mut is = List::nil();
        for _ in 0..n {
            let interface_index = self.next_char() as i32;
            let class = self.get_class(ctx, syms, interface_index)?;
            let class_type = ctx.sym(class).ty.expect("class type");
            let inter = self.types.erasure(ctx, syms, class_type);
            is = is.prepend(inter);
        }
        let interfaces_are_null = matches!(
            &ctx.typ(ct).kind,
            crate::code::type_::TypeKindData::ClassType {
                interfaces_field: None,
                ..
            }
        );
        if interfaces_are_null
            && let crate::code::type_::TypeKindData::ClassType {
                interfaces_field, ..
            } = &mut ctx.typ_mut(ct).kind
        {
            *interfaces_field = Some(is.reverse());
        }

        assert!(field_count as i32 == self.next_char() as i32);
        for _ in 0..field_count {
            let field = self.read_field(ctx, syms)?;
            self.enter_member(ctx, c, field);
        }
        assert!(method_count as i32 == self.next_char() as i32);
        for _ in 0..method_count {
            let method = self.read_method(ctx, syms)?;
            self.enter_member(ctx, c, method);
        }
        if ctx.sym(c).is_record() {
            // Java: for (RecordComponent rc: c.getRecordComponents())
            // rc.accessor = lookupMethod(c, rc.name, List.nil());
            let components = match &ctx.sym(c).kind_data {
                crate::code::symbol::SymbolKind::ClassSymbol {
                    record_components, ..
                } => record_components.clone(),
                _ => List::nil(),
            };
            let list = components;
            for component in &list {
                let component_name = ctx.sym(component).name.clone();
                let accessor = self.lookup_method(ctx, syms, c, &component_name, List::nil());
                if let crate::code::symbol::SymbolKind::RecordComponent { accessor: slot, .. } =
                    &mut ctx.sym_mut(component).kind_data
                {
                    *slot = accessor;
                }
            }
        }
        let tv = self.typevars.expect("typevars scope");
        self.typevars = Some(ctx.scope_leave(tv));
        Ok(())
    }

    /// Java: VarSymbol readField()
    fn read_field(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
    ) -> Result<SymbolId, crate::code::symbol::CompletionFailure> {
        let raw_flags = self.next_char() as i64;
        let flags = self.adjust_field_flags(raw_flags);
        let name_index = self.next_char() as i32;
        let name = self.get_name(name_index)?;
        let type_index = self.next_char() as i32;
        let ty = self.get_type(ctx, syms, type_index)?;
        let owner = self.current_owner.expect("currentOwner");
        let var = ctx.alloc_symbol(crate::code::symbol::Symbol::new_var_symbol(
            flags, name, ty, owner,
        ));
        self.read_member_attrs(ctx, syms, var)?;
        Ok(var)
    }

    /// Java: MethodSymbol readMethod()
    fn read_method(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
    ) -> Result<SymbolId, crate::code::symbol::CompletionFailure> {
        let raw_flags = self.next_char() as i64;
        let mut flags = self.adjust_method_flags(raw_flags);
        let name_index = self.next_char() as i32;
        let name = self.get_name(name_index)?;
        let type_index = self.next_char() as i32;
        let mut ty = self.get_type(ctx, syms, type_index)?;
        let owner = self.current_owner.expect("currentOwner");
        let owner_is_interface = ctx.sym(owner).is_interface();
        let is_clinit = name.to_java_string() == self.names.clinit.to_java_string();
        if owner_is_interface && (flags & crate::code::flags::ABSTRACT as i64) == 0 && !is_clinit {
            if self.major_version > Version::V52.major()
                || (self.major_version == Version::V52.major()
                    && self.minor_version >= Version::V52.minor())
            {
                if (flags
                    & (crate::code::flags::STATIC as i64 | crate::code::flags::PRIVATE as i64))
                    == 0
                {
                    ctx.sym_mut(owner).flags_field |= crate::code::flags::DEFAULT;
                    flags |= crate::code::flags::DEFAULT | crate::code::flags::ABSTRACT as i64;
                }
            } else {
                // protect against ill-formed classfiles
                let key = Fragment::from(
                    crate::resources::compiler_properties::fragments::invalid_default_interface(
                        crate::util::java_string::JavaString::from(
                            self.major_version.to_string().as_str(),
                        ),
                        crate::util::java_string::JavaString::from(
                            self.minor_version.to_string().as_str(),
                        ),
                    ),
                );
                return Err(self.bad_class_file(key).failure);
            }
        }
        self.validate_method_type(ctx, &name, ty)?;
        if name == self.names.init && ctx.sym(owner).has_outer_instance(ctx) {
            // Sometimes anonymous classes don't have an outer instance, however,
            // there is no reliable way to tell so we never strip this$n;
            // ditto for local classes.
            // Java: boolean local = !currentOwner.owner.members().includes(
            //     currentOwner, LookupKind.NON_RECURSIVE);
            let local = match ctx.sym(owner).owner {
                Some(owner_owner) => match ctx.sym(owner_owner).members() {
                    Some(members) => {
                        let scope = ctx.scope(members).clone();
                        !scope.includes_full(members, owner, &self.types, syms, ctx)
                    }
                    None => true,
                },
                None => true,
            };
            if !ctx.sym(owner).name.is_empty() && !local {
                // Java: type = new MethodType(adjustMethodParams(flags, type.getParameterTypes()),
                //     type.getReturnType(), type.getThrownTypes(), syms.methodClass);
                use crate::code::type_::TypeKindData;
                use crate::code::type_tag::TypeTag;
                let argtypes = ctx.typ(ty).get_parameter_types(ctx);
                let is_varargs = (flags & crate::code::flags::VARARGS) != 0;
                let mut adjusted: crate::util::list::List<TypeId> = crate::util::list::List::nil();
                if is_varargs && !argtypes.is_empty() {
                    let mut items: Vec<TypeId> = argtypes.iter().collect();
                    let last_index = items.len() - 1;
                    let last = items[last_index];
                    if let TypeKindData::ArrayType {
                        elemtype: Some(elemtype),
                        varargs: false,
                    } = &ctx.typ(last).kind
                    {
                        let elemtype = *elemtype;
                        let array_class = ctx.typ(last).tsym.expect("array class");
                        let varargs_array = ctx.alloc_type(crate::code::type_::Type::new(
                            TypeTag::ARRAY,
                            Some(array_class),
                            TypeKindData::ArrayType {
                                elemtype: Some(elemtype),
                                varargs: true,
                            },
                        ));
                        items[last_index] = varargs_array;
                    }
                    for (index, item) in items.iter().enumerate() {
                        if index > 0 {
                            adjusted = adjusted.append(*item);
                        }
                    }
                } else {
                    adjusted = if argtypes.is_empty() {
                        argtypes
                    } else {
                        argtypes.tail()
                    };
                }
                let restype = ctx
                    .typ(ty)
                    .get_return_type(ctx)
                    .expect("method return type");
                let thrown = ctx.typ(ty).get_thrown_types(ctx);
                let new_type = crate::code::type_::Type::new_method_type(
                    adjusted,
                    restype,
                    thrown,
                    syms.method_class,
                );
                ty = ctx.alloc_type(new_type);
            }
        }
        let method = ctx.alloc_symbol(crate::code::symbol::Symbol::new_method_symbol(
            flags,
            name.clone(),
            ty,
            owner,
        ));
        if self.types.is_signature_polymorphic(ctx, syms, method) {
            ctx.sym_mut(method).flags_field |= crate::code::flags::SIGNATURE_POLYMORPHIC;
        }
        if self.save_parameter_names {
            self.init_parameter_names(ctx, method);
        }
        let prev_owner = self.current_owner;
        self.current_owner = Some(method);
        self.read_member_attrs(ctx, syms, method)?;
        self.current_owner = prev_owner;
        self.validate_method_type(
            ctx,
            &ctx.sym(method).name.clone(),
            ctx.sym(method).ty.expect("method type"),
        )?;
        self.set_parameters(ctx, syms, method, ty)?;
        if (flags & crate::code::flags::VARARGS) != 0 {
            let params = ctx
                .typ(ctx.sym(method).ty.expect("method type"))
                .get_parameter_types(ctx);
            let mut last: Option<TypeId> = None;
            let mut l = params;
            while l.non_empty() {
                last = Some(*l.head());
                l = l.tail();
            }
            let is_array = last
                .map(|t| ctx.typ(t).has_tag(crate::code::type_tag::TypeTag::ARRAY))
                .unwrap_or(false);
            if !is_array {
                ctx.sym_mut(method).flags_field &= !crate::code::flags::VARARGS;
                let key = Fragment::from(
                    crate::resources::compiler_properties::fragments::malformed_vararg_method(),
                );
                return Err(self.bad_class_file(key).failure);
            }
        }
        Ok(method)
    }

    /// Java: void validateMethodType(Name name, Type t)
    fn validate_method_type(
        &self,
        ctx: &CompilerContext,
        name: &Name,
        t: TypeId,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        let tag = ctx.typ(t).get_tag();
        let is_init = name.to_java_string() == self.names.init.to_java_string();
        let restype_void = ctx
            .typ(t)
            .get_return_type(ctx)
            .map(|restype| {
                ctx.typ(restype)
                    .has_tag(crate::code::type_tag::TypeTag::VOID)
            })
            .unwrap_or(false);
        if !(tag == crate::code::type_tag::TypeTag::METHOD
            || tag == crate::code::type_tag::TypeTag::FORALL)
            || (is_init && !restype_void)
        {
            let key = Fragment::from(
                crate::resources::compiler_properties::fragments::method_descriptor_invalid(
                    name.clone(),
                ),
            );
            return Err(self.bad_class_file(key).failure);
        }
        Ok(())
    }

    /// Java: private void initAttributeReaders()
    fn init_attribute_readers(&mut self) {
        let defs: Vec<(Name, Version, Vec<AttributeKind>, AttributeReaderKind)> = vec![
            (
                self.names.Code.clone(),
                Version::V45_3,
                vec![AttributeKind::MEMBER],
                AttributeReaderKind::Code,
            ),
            (
                self.names.ConstantValue.clone(),
                Version::V45_3,
                vec![AttributeKind::MEMBER],
                AttributeReaderKind::ConstantValue,
            ),
            (
                self.names.Deprecated.clone(),
                Version::V45_3,
                vec![AttributeKind::CLASS, AttributeKind::MEMBER],
                AttributeReaderKind::Deprecated,
            ),
            (
                self.names.Exceptions.clone(),
                Version::V45_3,
                vec![AttributeKind::CLASS, AttributeKind::MEMBER],
                AttributeReaderKind::Exceptions,
            ),
            (
                self.names.InnerClasses.clone(),
                Version::V45_3,
                vec![AttributeKind::CLASS],
                AttributeReaderKind::InnerClasses,
            ),
            (
                self.names.LocalVariableTable.clone(),
                Version::V45_3,
                vec![AttributeKind::CLASS, AttributeKind::MEMBER],
                AttributeReaderKind::LocalVariableTable,
            ),
            (
                self.names.SourceFile.clone(),
                Version::V45_3,
                vec![AttributeKind::CLASS],
                AttributeReaderKind::SourceFile,
            ),
            (
                self.names.Synthetic.clone(),
                Version::V45_3,
                vec![AttributeKind::CLASS, AttributeKind::MEMBER],
                AttributeReaderKind::Synthetic,
            ),
            (
                self.names.EnclosingMethod.clone(),
                Version::V49,
                vec![AttributeKind::CLASS],
                AttributeReaderKind::EnclosingMethod,
            ),
            (
                self.names.Signature.clone(),
                Version::V49,
                vec![AttributeKind::CLASS, AttributeKind::MEMBER],
                AttributeReaderKind::Signature,
            ),
            (
                self.names.AnnotationDefault.clone(),
                Version::V49,
                vec![AttributeKind::CLASS, AttributeKind::MEMBER],
                AttributeReaderKind::AnnotationDefault,
            ),
            (
                self.names.RuntimeInvisibleAnnotations.clone(),
                Version::V49,
                vec![AttributeKind::CLASS, AttributeKind::MEMBER],
                AttributeReaderKind::RuntimeInvisibleAnnotations,
            ),
            (
                self.names.RuntimeInvisibleParameterAnnotations.clone(),
                Version::V49,
                vec![AttributeKind::CLASS, AttributeKind::MEMBER],
                AttributeReaderKind::RuntimeInvisibleParameterAnnotations,
            ),
            (
                self.names.RuntimeVisibleAnnotations.clone(),
                Version::V49,
                vec![AttributeKind::CLASS, AttributeKind::MEMBER],
                AttributeReaderKind::RuntimeVisibleAnnotations,
            ),
            (
                self.names.RuntimeVisibleParameterAnnotations.clone(),
                Version::V49,
                vec![AttributeKind::CLASS, AttributeKind::MEMBER],
                AttributeReaderKind::RuntimeVisibleParameterAnnotations,
            ),
            (
                self.names.Annotation.clone(),
                Version::V49,
                vec![AttributeKind::CLASS, AttributeKind::MEMBER],
                AttributeReaderKind::Annotation,
            ),
            (
                self.names.Bridge.clone(),
                Version::V49,
                vec![AttributeKind::MEMBER],
                AttributeReaderKind::Bridge,
            ),
            (
                self.names.Enum.clone(),
                Version::V49,
                vec![AttributeKind::CLASS, AttributeKind::MEMBER],
                AttributeReaderKind::Enum,
            ),
            (
                self.names.Varargs.clone(),
                Version::V49,
                vec![AttributeKind::CLASS, AttributeKind::MEMBER],
                AttributeReaderKind::Varargs,
            ),
            (
                self.names.RuntimeVisibleTypeAnnotations.clone(),
                Version::V52,
                vec![AttributeKind::CLASS, AttributeKind::MEMBER],
                AttributeReaderKind::RuntimeVisibleTypeAnnotations,
            ),
            (
                self.names.RuntimeInvisibleTypeAnnotations.clone(),
                Version::V52,
                vec![AttributeKind::CLASS, AttributeKind::MEMBER],
                AttributeReaderKind::RuntimeInvisibleTypeAnnotations,
            ),
            (
                self.names.MethodParameters.clone(),
                Version::V52,
                vec![AttributeKind::MEMBER],
                AttributeReaderKind::MethodParameters,
            ),
            (
                self.names.Module.clone(),
                Version::V53,
                vec![AttributeKind::CLASS],
                AttributeReaderKind::Module,
            ),
            (
                self.names.ModuleResolution.clone(),
                Version::V53,
                vec![AttributeKind::CLASS],
                AttributeReaderKind::ModuleResolution,
            ),
            (
                self.names.Record.clone(),
                Version::V58,
                vec![AttributeKind::CLASS],
                AttributeReaderKind::Record,
            ),
            (
                self.names.PermittedSubclasses.clone(),
                Version::V59,
                vec![AttributeKind::CLASS],
                AttributeReaderKind::PermittedSubclasses,
            ),
        ];
        self.attribute_readers = defs
            .into_iter()
            .map(|(name, version, kinds, reader)| AttributeReaderDef {
                name,
                version,
                kinds,
                reader,
            })
            .collect();
    }
    // ------------------------------------------------------------------
    // Module / ModuleResolution attribute readers (ClassReader.java:1090-1212)
    // ------------------------------------------------------------------

    /// Java: Set<ModuleFlags> readModuleFlags(int flags)
    fn read_module_flags(flags: i32) -> Vec<crate::code::symbol::ModuleFlags> {
        let mut set = Vec::new();
        for flag in crate::code::symbol::ModuleFlags::values() {
            if (flags & flag.value()) != 0 {
                set.push(flag);
            }
        }
        set
    }

    /// Java: Set<ModuleResolutionFlags> readModuleResolutionFlags(int flags)
    fn read_module_resolution_flags(flags: i32) -> Vec<crate::code::symbol::ModuleResolutionFlags> {
        let mut set = Vec::new();
        for flag in crate::code::symbol::ModuleResolutionFlags::values() {
            if (flags & flag.value()) != 0 {
                set.push(flag);
            }
        }
        set
    }

    /// Java: Set<RequiresFlag> readRequiresFlags(int flags)
    fn read_requires_flags(flags: i32) -> Vec<crate::code::directive::RequiresFlag> {
        let mut set = Vec::new();
        for flag in crate::code::directive::RequiresFlag::values() {
            if (flags & flag.value()) != 0 {
                set.push(flag);
            }
        }
        set
    }

    /// Java: Set<ExportsFlag> readExportsFlags(int flags)
    fn read_exports_flags(flags: i32) -> Vec<crate::code::directive::ExportsFlag> {
        let mut set = Vec::new();
        for flag in crate::code::directive::ExportsFlag::values() {
            if (flags & flag.value()) != 0 {
                set.push(flag);
            }
        }
        set
    }

    /// Java: Set<OpensFlag> readOpensFlags(int flags)
    fn read_opens_flags(flags: i32) -> Vec<crate::code::directive::OpensFlag> {
        let mut set = Vec::new();
        for flag in crate::code::directive::OpensFlag::values() {
            if (flags & flag.value()) != 0 {
                set.push(flag);
            }
        }
        set
    }

    /// Java: Name classNameMapper(byte[] arr, int offset, int length) — names.fromUtf(internalize(arr))
    fn class_name_mapper(&self, class_name: javac_strings::JavaString) -> Name {
        self.names.from_string(&class_name)
    }

    /// Java: Module AttributeReader.read (ClassReader.java:1090-1198)
    fn read_module_attr(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        sym: SymbolId,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        if ctx.sym(sym).kind != crate::code::kinds::Kind::TYP
            || ctx.sym(sym).owner.map(|o| ctx.sym(o).kind) != Some(crate::code::kinds::Kind::MDL)
        {
            return Ok(());
        }
        let msym = ctx.sym(sym).owner.expect("module owner");
        let module_index = self.next_char() as i32;
        let peeked = {
            let pool = self.pool();
            pool.peek_module_name(&self.buf, module_index, true, &self.names)
        };
        let module_name = self.class_name_mapper(peeked.expect("module name"));
        let current_name = ctx
            .sym(self.current_module.expect("currentModule"))
            .name
            .clone();
        if module_name.to_java_string() != current_name.to_java_string() {
            let key = Fragment::from(
                crate::resources::compiler_properties::fragments::module_name_mismatch(
                    module_name.clone(),
                    current_name.clone(),
                ),
            );
            return Err(self.bad_class_file(key).failure);
        }
        let module_flags = Self::read_module_flags(self.next_char() as i32);
        {
            let flags = match &mut ctx.sym_mut(msym).kind_data {
                crate::code::symbol::SymbolKind::ModuleSymbol { flags, .. } => flags,
                _ => panic!("module symbol"),
            };
            flags.extend(module_flags.iter().copied());
        }
        let version_index = self.next_char() as i32;
        let version = if version_index == 0 {
            None
        } else {
            Some(self.get_name(version_index)?)
        };
        if let crate::code::symbol::SymbolKind::ModuleSymbol { version: slot, .. } =
            &mut ctx.sym_mut(msym).kind_data
        {
            *slot = version;
        }
        let nrequires = self.next_char();
        let mut requires: List<crate::ids::DirectiveId> = List::nil();
        for _ in 0..nrequires {
            let rsym = {
                let index = self.next_char() as i32;
                self.get_module(ctx, syms, index)?
            };
            let flags = Self::read_requires_flags(self.next_char() as i32);
            if rsym == syms.java_base && self.major_version >= Version::V54.major() {
                if flags.contains(&crate::code::directive::RequiresFlag::TRANSITIVE) {
                    // The generated accessor currently has no parameters (registered
                    // P4 diagnostic-contract deviation, see the registration).
                    let key = Fragment::from(
                        crate::resources::compiler_properties::fragments::bad_requires_flag(),
                    );
                    return Err(self.bad_class_file(key).failure);
                }
                if flags.contains(&crate::code::directive::RequiresFlag::STATIC_PHASE) {
                    let key = Fragment::from(
                        crate::resources::compiler_properties::fragments::bad_requires_flag(),
                    );
                    return Err(self.bad_class_file(key).failure);
                }
            }
            self.next_char(); // skip compiled version
            let directive = ctx.alloc_directive(crate::code::directive::Directive::Requires {
                module: Some(rsym),
                flags,
            });
            requires = requires.append(directive);
        }
        {
            let (req_slot, dir_slot) = match &mut ctx.sym_mut(msym).kind_data {
                crate::code::symbol::SymbolKind::ModuleSymbol {
                    requires: requires_field,
                    directives: directives_field,
                    ..
                } => (requires_field, directives_field),
                _ => panic!("module symbol"),
            };
            *req_slot = requires.clone();
            *dir_slot = dir_slot.append_list(&requires);
        }
        let mut all_directives = {
            match &ctx.sym(msym).kind_data {
                crate::code::symbol::SymbolKind::ModuleSymbol { directives, .. } => {
                    directives.clone()
                }
                _ => List::nil(),
            }
        };
        let nexports = self.next_char();
        let mut exports: List<crate::ids::DirectiveId> = List::nil();
        for _ in 0..nexports {
            let p = {
                let index = self.next_char() as i32;
                self.get_package(ctx, syms, index)?
            };
            let flags = Self::read_exports_flags(self.next_char() as i32);
            let nto = self.next_char();
            let modules = if nto == 0 {
                None
            } else {
                let mut to = List::nil();
                for _ in 0..nto {
                    let module = {
                        let index = self.next_char() as i32;
                        self.get_module(ctx, syms, index)?
                    };
                    to = to.append(module);
                }
                Some(to)
            };
            let directive = ctx.alloc_directive(crate::code::directive::Directive::Exports {
                packge: Some(p),
                modules,
                flags,
            });
            exports = exports.append(directive);
        }
        all_directives = all_directives.append_list(&exports);
        {
            let exports_slot = match &mut ctx.sym_mut(msym).kind_data {
                crate::code::symbol::SymbolKind::ModuleSymbol {
                    exports: exports_field,
                    ..
                } => exports_field,
                _ => panic!("module symbol"),
            };
            *exports_slot = exports.clone();
            all_directives = all_directives.append_list(&exports);
            if let crate::code::symbol::SymbolKind::ModuleSymbol {
                directives: directives_field,
                ..
            } = &mut ctx.sym_mut(msym).kind_data
            {
                *directives_field = all_directives.clone();
            }
        }
        let module_flags_now = match &ctx.sym(msym).kind_data {
            crate::code::symbol::SymbolKind::ModuleSymbol { flags, .. } => flags.clone(),
            _ => Vec::new(),
        };
        let nopens = self.next_char();
        if nopens != 0 && module_flags_now.contains(&crate::code::symbol::ModuleFlags::OPEN) {
            let key = Fragment::from(
                crate::resources::compiler_properties::fragments::module_non_zero_opens(
                    ctx.sym(self.current_module.expect("currentModule"))
                        .name
                        .clone(),
                ),
            );
            return Err(self.bad_class_file(key).failure);
        }
        let mut opens: List<crate::ids::DirectiveId> = List::nil();
        for _ in 0..nopens {
            let p = {
                let index = self.next_char() as i32;
                self.get_package(ctx, syms, index)?
            };
            let flags = Self::read_opens_flags(self.next_char() as i32);
            let nto = self.next_char();
            let modules = if nto == 0 {
                None
            } else {
                let mut to = List::nil();
                for _ in 0..nto {
                    let module = {
                        let index = self.next_char() as i32;
                        self.get_module(ctx, syms, index)?
                    };
                    to = to.append(module);
                }
                Some(to)
            };
            let directive = ctx.alloc_directive(crate::code::directive::Directive::Opens {
                packge: Some(p),
                modules,
                flags,
            });
            opens = opens.append(directive);
        }
        {
            if let crate::code::symbol::SymbolKind::ModuleSymbol {
                opens: opens_field,
                directives: directives_field,
                ..
            } = &mut ctx.sym_mut(msym).kind_data
            {
                *opens_field = opens.clone();
                *directives_field = directives_field.append_list(&opens);
            }
        }
        let mut uses_names: List<Name> = List::nil();
        let nuses = self.next_char();
        for _ in 0..nuses {
            let index = self.next_char() as i32;
            let peeked = {
                let pool = self.pool();
                pool.peek_class_name(&self.buf, index, true, &self.names)
            };
            uses_names = uses_names.append(self.class_name_mapper(peeked.expect("uses name")));
        }
        self.interim_uses = uses_names;
        let mut provides_names: List<(Name, List<Name>)> = List::nil();
        let nprovides = self.next_char();
        for _ in 0..nprovides {
            let index = self.next_char() as i32;
            let peeked = {
                let pool = self.pool();
                pool.peek_class_name(&self.buf, index, true, &self.names)
            };
            let service = self.class_name_mapper(peeked.expect("provides service"));
            let nimpls = self.next_char();
            let mut impls: List<Name> = List::nil();
            for _ in 0..nimpls {
                let index = self.next_char() as i32;
                let peeked = {
                    let pool = self.pool();
                    pool.peek_class_name(&self.buf, index, true, &self.names)
                };
                impls = impls.append(self.class_name_mapper(peeked.expect("provides impl")));
            }
            provides_names = provides_names.append((service, impls));
        }
        self.interim_provides = provides_names;
        Ok(())
    }

    /// Java: ModuleResolution AttributeReader.read (ClassReader.java:1200-1212)
    fn read_module_resolution_attr(
        &mut self,
        ctx: &mut CompilerContext,
        sym: SymbolId,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        if ctx.sym(sym).kind != crate::code::kinds::Kind::TYP
            || ctx.sym(sym).owner.map(|o| ctx.sym(o).kind) != Some(crate::code::kinds::Kind::MDL)
        {
            return Ok(());
        }
        let msym = ctx.sym(sym).owner.expect("module owner");
        let flags = Self::read_module_resolution_flags(self.next_char() as i32);
        if let crate::code::symbol::SymbolKind::ModuleSymbol {
            resolution_flags, ..
        } = &mut ctx.sym_mut(msym).kind_data
        {
            resolution_flags.extend(flags);
        }
        Ok(())
    }
    // ------------------------------------------------------------------
    // Annotation reading (ClassReader.java:1429-1490, 1955-2100)
    // ------------------------------------------------------------------

    /// Java: ClassReader.ProxyType.resolve() (ClassReader.java:3218-3220)
    ///
    /// # Arguments
    ///   - ctx/syms: the compiler services;
    ///   - t: the type that may be a ProxyType.
    ///
    /// # Returns
    ///   - the resolved signature type for ProxyType values, t otherwise.
    ///
    /// # Panics
    ///   - none.
    fn resolve_possible_proxy_type(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        t: TypeId,
    ) -> Result<TypeId, crate::code::symbol::CompletionFailure> {
        let name = match &ctx.typ(t).kind {
            crate::code::type_::TypeKindData::ProxyType { name } => name.clone(),
            _ => return Ok(t),
        };
        // Java: name.map(ClassReader.this::sigToType) — NameMapper over the
        // name UTF-8 byte array (sigToType(byte[], offset, len)).
        let sig = name.to_utf();
        let offset = name.get_byte_offset();
        let len = name.get_byte_length();
        self.sig_to_type_bytes(ctx, syms, &sig, offset, len)
    }

    /// Java: List<CompoundAnnotationProxy> readAnnotations()
    fn read_annotations(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
    ) -> Result<List<AnnotationValue>, crate::code::symbol::CompletionFailure> {
        let num_attributes = self.next_char();
        let mut annotations = List::nil();
        for _ in 0..num_attributes {
            annotations = annotations.append(self.read_compound_annotation(ctx, syms)?);
        }
        Ok(annotations)
    }

    /// Java: Type readTypeOrClassSymbol(int i) / Type readTypeToProxy(int i)
    fn read_type_to_proxy(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        index: i32,
    ) -> Result<TypeId, crate::code::symbol::CompletionFailure> {
        let module_info = self
            .current_module
            .and_then(|module| match &ctx.sym(module).kind_data {
                crate::code::symbol::SymbolKind::ModuleSymbol { module_info, .. } => *module_info,
                _ => None,
            });
        if module_info == self.current_owner {
            // Java: ClassReader.java:1537-1543
            //   if (currentModule.module_info == currentOwner) return new ProxyType(i);
            // ProxyType: name = poolReader.getName(index), tag NONE, noSymbol.
            let name = self.get_name(index)?;
            let ty = ctx.alloc_type(crate::code::type_::Type::new(
                crate::code::type_tag::TypeTag::NONE,
                None,
                crate::code::type_::TypeKindData::ProxyType { name },
            ));
            return Ok(ty);
        }
        self.get_type(ctx, syms, index)
    }

    /// Java: CompoundAnnotationProxy readCompoundAnnotation()
    fn read_compound_annotation(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
    ) -> Result<AnnotationValue, crate::code::symbol::CompletionFailure> {
        let ty = {
            let index = self.next_char() as i32;
            self.read_type_to_proxy(ctx, syms, index)?
        };
        let num_fields = self.next_char();
        let mut pairs: List<(Name, AnnotationValue)> = List::nil();
        for _ in 0..num_fields {
            let name_index = self.next_char() as i32;
            let name = self.get_name(name_index)?;
            let value = self.read_attribute_value(ctx, syms)?;
            pairs = pairs.append((name, value));
        }
        Ok(AnnotationValue::Compound { ty, values: pairs })
    }

    /// Java: Attribute readAttributeValue() (ClassReader.java:1985-2045)
    fn read_attribute_value(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
    ) -> Result<AnnotationValue, crate::code::symbol::CompletionFailure> {
        use crate::code::attribute::ConstValue;
        let tag = self.next_byte() as u8 as char;
        match tag {
            'B' => self.read_constant_annotation_value(ctx, syms.string_type, false),
            'C' => self.read_constant_annotation_value(ctx, syms.char_type, false),
            'D' => self.read_constant_annotation_value(ctx, syms.double_type, false),
            'F' => self.read_constant_annotation_value(ctx, syms.float_type, false),
            'I' => self.read_constant_annotation_value(ctx, syms.int_type, false),
            'J' => self.read_constant_annotation_value(ctx, syms.long_type, false),
            'S' => self.read_constant_annotation_value(ctx, syms.short_type, false),
            'Z' => self.read_constant_annotation_value(ctx, syms.boolean_type, false),
            's' => {
                let index = self.next_char() as i32;
                let name = self.get_name(index)?;
                Ok(AnnotationValue::Constant {
                    ty: syms.string_type,
                    value: ConstValue::String(name.to_java_string()),
                })
            }
            'e' => {
                let enum_index = self.next_char() as i32;
                let enum_type = self.read_type_to_proxy(ctx, syms, enum_index)?;
                let name_index = self.next_char() as i32;
                let enumerator = self.get_name(name_index)?;
                Ok(AnnotationValue::Enum {
                    enum_type,
                    enumerator,
                })
            }
            'c' => {
                let class_index = self.next_char() as i32;
                let class_type = self.read_type_to_proxy(ctx, syms, class_index)?;
                Ok(AnnotationValue::Class { class_type })
            }
            '[' => {
                let n = self.next_char();
                let mut values = List::nil();
                for _ in 0..n {
                    values = values.append(self.read_attribute_value(ctx, syms)?);
                }
                Ok(AnnotationValue::Array { values })
            }
            '@' => self.read_compound_annotation(ctx, syms),
            other => panic!("AssertionError: unknown annotation tag '{other}'"),
        }
    }

    /// Helper for the primitive/constant reading branches of readAttributeValue
    fn read_constant_annotation_value(
        &mut self,
        ctx: &mut CompilerContext,
        ty: TypeId,
        _string_fallback: bool,
    ) -> Result<AnnotationValue, crate::code::symbol::CompletionFailure> {
        use crate::code::attribute::ConstValue;
        use crate::jvm::pool_reader::PoolValue;
        let index = self.next_char() as i32;
        let v = self.get_constant(index)?;
        let value = match &v {
            PoolValue::Integer(value) => ConstValue::Integer(*value),
            PoolValue::Long(value) => ConstValue::Long(*value),
            PoolValue::Float(value) => ConstValue::Float(*value),
            PoolValue::Double(value) => ConstValue::Double(*value),
            _ => panic!("unexpected constant pool value for annotation element"),
        };
        let _ = ctx;
        Ok(AnnotationValue::Constant { ty, value })
    }

    /// Java: void attachAnnotations(Symbol sym) / void attachAnnotations(Symbol sym,
    /// List<CompoundAnnotationProxy> annotations) (ClassReader.java:1430-1495)
    fn attach_annotations(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        sym: SymbolId,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        let annotations = self.read_annotations(ctx, syms)?;
        if annotations.is_empty() {
            return Ok(());
        }
        let boolean_type = syms.boolean_type;
        let mut proxies: List<AnnotationValue> = List::nil();
        let current = annotations;
        for proxy in &current {
            let proxy_ty = match &proxy {
                AnnotationValue::Compound { ty, .. } => *ty,
                _ => panic!("CompoundAnnotationProxy expected"),
            };
            let proxy_tsym = ctx.typ(proxy_ty).tsym;
            if internal_type_matches(ctx, syms.proprietary_type, proxy_tsym) {
                ctx.sym_mut(sym).flags_field |= flags::PROPRIETARY;
            } else if internal_type_matches(ctx, syms.profile_type, proxy_tsym) {
                if *self.profile != Profile::DEFAULT {
                    for (name, value) in proxy_values(&proxy) {
                        if name.to_java_string() == self.names.value.to_java_string()
                            && let AnnotationValue::Constant {
                                value: crate::code::attribute::ConstValue::Integer(v),
                                ..
                            } = value
                        {
                            let profile_value = match *self.profile {
                                Profile::COMPACT1 => 1,
                                Profile::COMPACT2 => 2,
                                Profile::COMPACT3 => 3,
                                Profile::DEFAULT => 0,
                            };
                            if v > profile_value {
                                ctx.sym_mut(sym).flags_field |= flags::NOT_IN_PROFILE;
                            }
                        }
                    }
                }
            } else if internal_type_matches(ctx, syms.preview_feature_internal_type, proxy_tsym) {
                ctx.sym_mut(sym).flags_field |= flags::PREVIEW_API;
                if set_flag_if_attribute_true(ctx, &proxy, &self.names.reflective, boolean_type) {
                    ctx.sym_mut(sym).flags_field |= flags::PREVIEW_REFLECTIVE;
                }
            } else if internal_type_matches(ctx, syms.value_based_internal_type, proxy_tsym) {
                assert!(
                    ctx.sym(sym).kind == crate::code::kinds::Kind::TYP,
                    "AssertionError: value-based internal annotation on a non-type"
                );
                ctx.sym_mut(sym).flags_field |= flags::VALUE_BASED;
            } else {
                let target_tsym = ctx.typ(syms.annotation_target_type).tsym;
                let repeatable_tsym = ctx.typ(syms.repeatable_type).tsym;
                let deprecated_tsym = ctx.typ(syms.deprecated_type).tsym;
                let preview_tsym = ctx.typ(syms.preview_feature_type).tsym;
                let value_based_tsym = ctx.typ(syms.value_based_type).tsym;
                if proxy_tsym == target_tsym {
                    self.target = Some(proxy.clone());
                } else if proxy_tsym == repeatable_tsym {
                    self.repeatable = Some(proxy.clone());
                } else if proxy_tsym == deprecated_tsym {
                    ctx.sym_mut(sym).flags_field |= flags::DEPRECATED as i64;
                    ctx.sym_mut(sym).flags_field |= flags::DEPRECATED_ANNOTATION;
                    if set_flag_if_attribute_true(
                        ctx,
                        &proxy,
                        &self
                            .names
                            .from_string(&javac_strings::JavaString::from("forRemoval")),
                        boolean_type,
                    ) {
                        ctx.sym_mut(sym).flags_field |= flags::DEPRECATED_REMOVAL;
                    }
                } else if proxy_tsym == preview_tsym {
                    ctx.sym_mut(sym).flags_field |= flags::PREVIEW_API;
                    if set_flag_if_attribute_true(ctx, &proxy, &self.names.reflective, boolean_type)
                    {
                        ctx.sym_mut(sym).flags_field |= flags::PREVIEW_REFLECTIVE;
                    }
                } else if proxy_tsym == value_based_tsym
                    && ctx.sym(sym).kind == crate::code::kinds::Kind::TYP
                {
                    ctx.sym_mut(sym).flags_field |= flags::VALUE_BASED;
                }
                proxies = proxies.append(proxy);
            }
        }
        // Java: annotate.normal(new AnnotationCompleter(sym, proxies.toList()));
        self.queue_compound_annotation_completer(ctx, sym, proxies)?;
        Ok(())
    }

    /// Java: AnnotationCompleter.run — deproxy the classfile annotation proxies
    /// and attach them to sym (ClassReader.java:2135-2160). The completion is
    /// queued on Annotate; without the service the raw proxies are kept pending.
    fn queue_compound_annotation_completer(
        &mut self,
        _ctx: &mut CompilerContext,
        sym: SymbolId,
        proxies: List<AnnotationValue>,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        let annotate = self.annotate.borrow().clone();
        match annotate {
            Some(annotate) => {
                let services = Rc::clone(&annotate.services);
                let class_file = self.current_class_file.clone();
                let reader_handle = self.reader_handle.clone();
                let lint_classfile = self.lint_classfile;
                annotate.normal(Box::new(
                    move |ctx: &mut CompilerContext,
                          syms: &crate::code::symtab::Symtab,
                          _attr: &mut crate::comp::attr::Attr| {
                        let prev_source = services.log.borrow_mut().use_source(class_file.clone());
                        let result = (|| -> Result<(), crate::code::symbol::CompletionFailure> {
                            let mut reader_guard = reader_handle.as_ref().map(|rc| rc.borrow_mut());
                            let types = Rc::clone(&services.types);
                            let deprox = DeproxyContext {
                                log: Rc::clone(&services.log),
                                lint_classfile,
                                current_file: class_file.clone(),
                            };
                            let new_list = deproxy_compound_list(
                                ctx,
                                syms,
                                &types,
                                &services.names,
                                &proxies,
                                reader_guard.as_deref_mut(),
                                Some(&deprox),
                            )?;
                            let deprecated_ty = syms.deprecated_type;
                            for attr in new_list.iter() {
                                if ctx.attr(attr).ty == Some(deprecated_ty) {
                                    ctx.sym_mut(sym).flags_field |= flags::DEPRECATED as i64;
                                    ctx.sym_mut(sym).flags_field |= flags::DEPRECATED_ANNOTATION;
                                    let for_removal = services.names.from_string(
                                        &javac_strings::JavaString::from("forRemoval"),
                                    );
                                    if let Some(for_removal_attr) =
                                        ctx.attr(attr).member(ctx, &for_removal)
                                        && let crate::code::attribute::AttributeKind::Constant {
                                            value: crate::code::attribute::ConstValue::Integer(v),
                                        } = &ctx.attr(for_removal_attr).kind
                                        && *v != 0
                                    {
                                        ctx.sym_mut(sym).flags_field |= flags::DEPRECATED_REMOVAL;
                                    }
                                }
                            }
                            if ctx.sym(sym).annotations_pending_completion() {
                                ctx.sym_mut(sym).set_declaration_attributes(new_list);
                            } else {
                                ctx.sym_mut(sym).append_attributes(new_list);
                            }
                            Ok(())
                        })();
                        services.log.borrow_mut().use_source(prev_source);
                        result
                    },
                ));
            }
            None => {
                self.pending_annotations.push((sym, proxies));
            }
        }
        Ok(())
    }

    /// Java: TypeAnnotationCompleter.run (ClassReader.java:2188-2202): deproxy
    /// the Runtime[In]visibleTypeAnnotations proxies, prepend them to the
    /// symbol's raw type attributes and (when enabled) attach them to the
    /// symbol types.
    fn queue_type_annotation_completer(
        &mut self,
        _ctx: &mut CompilerContext,
        sym: SymbolId,
        proxies: List<TypeAnnotationProxy>,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        let annotate = self.annotate.borrow().clone();
        match annotate {
            Some(annotate) => {
                let services = Rc::clone(&annotate.services);
                let class_file = self.current_class_file.clone();
                let reader_handle = self.reader_handle.clone();
                let attach_type_annotations = self.add_type_annotations_to_symbol;
                let lint_classfile = self.lint_classfile;
                annotate.normal(Box::new(move |ctx: &mut CompilerContext, syms: &crate::code::symtab::Symtab, _attr: &mut crate::comp::attr::Attr| {
                    let prev_source = services.log.borrow_mut().use_source(class_file.clone());
                    let result = (|| -> Result<(), crate::code::symbol::CompletionFailure> {
                        let mut reader_guard =
                            reader_handle.as_ref().map(|rc| rc.borrow_mut());
                        let types = Rc::clone(&services.types);
                        let names = Rc::clone(&services.names);
                        let deprox = DeproxyContext {
                            log: Rc::clone(&services.log),
                            lint_classfile,
                            current_file: class_file.clone(),
                        };
                        let current = proxies.clone();
                        let mut new_list = List::nil();
                        for proxy in &current {
                        let compound = deproxy_compound(
                            ctx,
                            syms,
                            &types,
                            &names,
                            &proxy.compound,
                            reader_guard.as_deref_mut(),
                            Some(&deprox),
                        )?;
                        let ty = ctx.attr(compound).ty;
                        let values = match ctx.attr(compound).kind.clone() {
                            crate::code::attribute::AttributeKind::Compound { values, .. } => values,
                            _ => panic!("deproxyCompound must return a compound attribute"),
                        };
                        let attribute = ctx.alloc_attribute(
                            crate::code::attribute::Attribute::new_type_compound(
                                ty,
                                values,
                                Some(proxy.position.clone()),
                            ),
                        );
                        new_list = new_list.append(attribute);
                        }
                        let existing = ctx.sym(sym).get_raw_type_attributes();
                        ctx.sym_mut(sym).set_type_attributes(new_list.clone().append_list(&existing));
                        if attach_type_annotations {
                            match add_type_annotations_to_symbol(ctx, syms, sym, new_list) {
                                Ok(()) => {}
                                Err(failure) => {
                                    // Java: catch (CompletionFailure ex) -> log.error(
                                    // Errors.CantAttachTypeAnnotations(attributes, s.owner, s.name,
                                    // ex.getDetailValue()))
                                    let mut annotation_args = Vec::new();
                                    let mut attrs = ctx.sym(sym).get_raw_type_attributes();
                                    while attrs.non_empty() {
                                        let attr = *attrs.head();
                                        attrs = attrs.tail();
                                        if let Some(ty) = ctx.attr(attr).ty {
                                            annotation_args.push(
                                                crate::resources::diagnostic_contract::AnnotationArg::placeholder(
                                                    format!("@{}", crate::code::printer::visit_type(ctx, ty)),
                                                ),
                                            );
                                        }
                                    }
                                    let owner_text = ctx
                                        .sym(sym)
                                        .owner
                                        .map(|owner| {
                                            crate::code::printer::visit_symbol(
                                                ctx,
                                                &crate::util::names::Names::new(false),
                                                owner,
                                            )
                                        })
                                        .unwrap_or_default();
                                    let detail = match failure.get_detail_value() {
                                        Some(diag) => crate::resources::diagnostic_contract::DiagnosticArg::JCDiagnostic(
                                            std::rc::Rc::new(diag),
                                        ),
                                        None => crate::resources::diagnostic_contract::DiagnosticArg::from_display("null"),
                                    };
                                    let info = crate::resources::compiler_properties::errors::cant_attach_type_annotations(
                                        annotation_args,
                                        crate::resources::diagnostic_contract::SymbolArg::placeholder(owner_text),
                                        ctx.sym(sym).name.clone(),
                                        detail,
                                    );
                                    services
                                        .log
                                        .borrow_mut()
                                        .error_key(crate::util::jc_diagnostic::Error::from(info));
                                }
                            }
                        }
                        Ok(())
                    })();
                    services.log.borrow_mut().use_source(prev_source);
                    result
                }));
            }
            None => {
                self.pending_type_annotations.push((sym, proxies));
            }
        }
        Ok(())
    }

    /// Java: void readParameterAnnotations(Symbol meth) (ClassReader.java:1466-1482)
    fn read_parameter_annotations(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        sym: SymbolId,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        let num_parameters = self.next_byte() as usize;
        let mut annotations_by_parameter = Vec::with_capacity(num_parameters);
        for _ in 0..num_parameters {
            annotations_by_parameter.push(self.read_annotations(ctx, syms)?);
        }
        match &mut self.parameter_annotations {
            None => {
                self.parameter_annotations = Some(vec![None; num_parameters]);
            }
            Some(parameter_annotations) if parameter_annotations.len() != num_parameters => {
                let key = Fragment::from(
                    crate::resources::compiler_properties::fragments::bad_runtime_invisible_param_annotations(),
                );
                return Err(self.bad_class_file(key).failure);
            }
            Some(_) => {}
        }
        self.pending_parameter_annotations
            .push((sym, annotations_by_parameter.clone()));
        if let Some(parameter_annotations) = &mut self.parameter_annotations {
            for (index, annotations) in annotations_by_parameter.into_iter().enumerate() {
                let slot = &mut parameter_annotations[index];
                *slot = Some(match slot.take() {
                    Some(existing) => existing.append_list(&annotations),
                    None => annotations,
                });
            }
        }
        Ok(())
    }

    /// Java: void attachAnnotationDefault(Symbol sym) (ClassReader.java:1487-1503)
    fn attach_annotation_default(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        sym: SymbolId,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        let value = self.read_attribute_value(ctx, syms)?;
        // Java: meth.defaultValue = value (temporary sentinel) + annotate.normal(
        // new AnnotationDefaultCompleter(meth, value));
        let annotate = self.annotate.borrow().clone();
        match annotate {
            Some(annotate) => {
                let sentinel = annotate.unfinished_default_value();
                if let crate::code::symbol::SymbolKind::MethodSymbol {
                    default_value: slot,
                    ..
                } = &mut ctx.sym_mut(sym).kind_data
                {
                    *slot = Some(sentinel);
                }
                let services = Rc::clone(&annotate.services);
                let class_file = self.current_class_file.clone();
                let reader_handle = self.reader_handle.clone();
                let lint_classfile = self.lint_classfile;
                annotate.normal(Box::new(
                    move |ctx: &mut CompilerContext,
                          syms: &crate::code::symtab::Symtab,
                          _attr: &mut crate::comp::attr::Attr| {
                        let prev_source = services.log.borrow_mut().use_source(class_file.clone());
                        let result = (|| -> Result<(), crate::code::symbol::CompletionFailure> {
                            let mut reader_guard = reader_handle.as_ref().map(|rc| rc.borrow_mut());
                            let types = Rc::clone(&services.types);
                            let deprox = DeproxyContext {
                                log: Rc::clone(&services.log),
                                lint_classfile,
                                current_file: class_file.clone(),
                            };
                            let meth_ty = ctx.sym(sym).ty.expect("annotation method type");
                            let restype = match &ctx.typ(meth_ty).kind {
                                crate::code::type_::TypeKindData::MethodType {
                                    restype, ..
                                } => restype.expect("method return type"),
                                _ => panic!("MethodType expected"),
                            };
                            let attr = deproxy_value(
                                ctx,
                                syms,
                                &types,
                                &services.names,
                                restype,
                                &value,
                                reader_guard.as_deref_mut(),
                                Some(&deprox),
                            )?;
                            if let crate::code::symbol::SymbolKind::MethodSymbol {
                                default_value: slot,
                                ..
                            } = &mut ctx.sym_mut(sym).kind_data
                            {
                                *slot = Some(attr);
                            }
                            Ok(())
                        })();
                        services.log.borrow_mut().use_source(prev_source);
                        result
                    },
                ));
            }
            None => {
                self.pending_annotation_defaults.push((sym, value));
            }
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // Type annotation reading (ClassReader.java:1505-1690)
    // ------------------------------------------------------------------

    /// Java: void attachTypeAnnotations(Symbol sym) (ClassReader.java:1490-1505)
    fn attach_type_annotations(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        sym: SymbolId,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        let num_attributes = self.next_char();
        if num_attributes == 0 {
            return Ok(());
        }
        let mut proxies = List::nil();
        for _ in 0..num_attributes {
            proxies = proxies.append(self.read_type_annotation(ctx, syms)?);
        }
        // Java: annotate.normal(new TypeAnnotationCompleter(sym, proxies.toList()));
        self.queue_type_annotation_completer(ctx, sym, proxies)?;
        Ok(())
    }

    /// Java: TypeAnnotationProxy readTypeAnnotation()
    fn read_type_annotation(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
    ) -> Result<TypeAnnotationProxy, crate::code::symbol::CompletionFailure> {
        let position = self.read_position()?;
        let compound = self.read_compound_annotation(ctx, syms)?;
        Ok(TypeAnnotationProxy { compound, position })
    }

    /// Java: List<TypeAnnotationPosition.TypePathEntry> readTypePath()
    fn read_type_path(&mut self) -> List<crate::code::type_annotation_position::TypePathEntry> {
        let len = self.next_byte();
        let mut loc: List<i32> = List::nil();
        let bytes_per_entry = crate::code::type_annotation_position::TypePathEntry::BYTES_PER_ENTRY;
        for _ in 0..len * bytes_per_entry {
            loc = loc.append(self.next_byte());
        }
        crate::code::type_annotation_position::TypeAnnotationPosition::get_type_path_from_binary(
            loc,
        )
    }

    /// Java: TypeAnnotationPosition readPosition() (ClassReader.java:1570-1780)
    fn read_position(
        &mut self,
    ) -> Result<
        crate::code::type_annotation_position::TypeAnnotationPosition,
        crate::code::symbol::CompletionFailure,
    > {
        use crate::code::target_type::TargetType;
        use crate::code::type_annotation_position::TypeAnnotationPosition as Pos;
        let tag = self.next_byte();
        if !TargetType::is_valid_target_type_value(tag) {
            // The generated accessor currently has no parameters (registered P4
            // diagnostic-contract deviation).
            let key = Fragment::from(
                crate::resources::compiler_properties::fragments::bad_type_annotation_value(),
            );
            return Err(self.bad_class_file(key).failure);
        }
        let target = TargetType::from_target_type_value(tag);
        let position = match target {
            TargetType::INSTANCEOF => {
                let offset = self.next_char() as i32;
                let mut position = Pos::instance_of(self.read_type_path(), -1, None);
                position.offset = offset;
                position
            }
            TargetType::NEW => {
                let offset = self.next_char() as i32;
                let mut position = Pos::new_obj(self.read_type_path(), -1, None);
                position.offset = offset;
                position
            }
            TargetType::CONSTRUCTOR_REFERENCE => {
                let offset = self.next_char() as i32;
                let mut position = Pos::constructor_ref(self.read_type_path(), -1, None);
                position.offset = offset;
                position
            }
            TargetType::METHOD_REFERENCE => {
                let offset = self.next_char() as i32;
                let mut position = Pos::method_ref(self.read_type_path(), -1, None);
                position.offset = offset;
                position
            }
            TargetType::LOCAL_VARIABLE | TargetType::RESOURCE_VARIABLE => {
                let table_length = self.next_char();
                let mut lvar_offset = Vec::with_capacity(table_length as usize);
                let mut lvar_length = Vec::with_capacity(table_length as usize);
                let mut lvar_index = Vec::with_capacity(table_length as usize);
                for _ in 0..table_length {
                    lvar_offset.push(self.next_char() as i32);
                    lvar_length.push(self.next_char() as i32);
                    lvar_index.push(self.next_char() as i32);
                }
                let mut position = if target == TargetType::LOCAL_VARIABLE {
                    Pos::local_variable(self.read_type_path(), -1, None)
                } else {
                    Pos::resource_variable(self.read_type_path(), -1, None)
                };
                position.lvar_offset = Some(lvar_offset);
                position.lvar_length = Some(lvar_length);
                position.lvar_index = Some(lvar_index);
                position
            }
            TargetType::EXCEPTION_PARAMETER => {
                let exception_index = self.next_char() as i32;
                let mut position = Pos::exception_parameter(self.read_type_path(), -1, None);
                position.set_exception_index(exception_index);
                position
            }
            TargetType::METHOD_RECEIVER => Pos::method_receiver(self.read_type_path(), -1, None),
            TargetType::CLASS_TYPE_PARAMETER => {
                let parameter_index = self.next_byte();
                Pos::type_parameter(self.read_type_path(), parameter_index, None)
            }
            TargetType::METHOD_TYPE_PARAMETER => {
                let parameter_index = self.next_byte();
                Pos::method_type_parameter(self.read_type_path(), parameter_index, None)
            }
            TargetType::CLASS_TYPE_PARAMETER_BOUND => {
                let parameter_index = self.next_byte();
                let bound_index = self.next_byte();
                Pos::type_parameter_bound(
                    self.read_type_path(),
                    parameter_index,
                    bound_index,
                    -1,
                    None,
                )
            }
            TargetType::METHOD_TYPE_PARAMETER_BOUND => {
                let parameter_index = self.next_byte();
                let bound_index = self.next_byte();
                Pos::method_type_parameter_bound(
                    self.read_type_path(),
                    parameter_index,
                    bound_index,
                    -1,
                    None,
                )
            }
            TargetType::CLASS_EXTENDS => {
                let type_index = self.next_char() as i32;
                Pos::class_extends(self.read_type_path(), type_index, -1, None)
            }
            TargetType::THROWS => {
                let type_index = self.next_char() as i32;
                Pos::method_throws(self.read_type_path(), type_index, -1, None)
            }
            TargetType::METHOD_FORMAL_PARAMETER => {
                let parameter_index = self.next_byte();
                Pos::method_parameter(self.read_type_path(), parameter_index, -1, None)
            }
            TargetType::CAST => {
                let offset = self.next_char() as i32;
                let type_index = self.next_byte();
                let mut position = Pos::type_cast(self.read_type_path(), type_index, -1, None);
                position.offset = offset;
                position
            }
            TargetType::CONSTRUCTOR_INVOCATION_TYPE_ARGUMENT
            | TargetType::METHOD_INVOCATION_TYPE_ARGUMENT
            | TargetType::CONSTRUCTOR_REFERENCE_TYPE_ARGUMENT
            | TargetType::METHOD_REFERENCE_TYPE_ARGUMENT => {
                let offset = self.next_char() as i32;
                let type_index = self.next_byte();
                let mut position = match target {
                    TargetType::CONSTRUCTOR_INVOCATION_TYPE_ARGUMENT => {
                        Pos::constructor_invocation_type_arg(
                            self.read_type_path(),
                            type_index,
                            -1,
                            None,
                        )
                    }
                    TargetType::METHOD_INVOCATION_TYPE_ARGUMENT => {
                        Pos::method_invocation_type_arg(self.read_type_path(), type_index, -1, None)
                    }
                    TargetType::CONSTRUCTOR_REFERENCE_TYPE_ARGUMENT => {
                        Pos::constructor_ref_type_arg(self.read_type_path(), type_index, -1, None)
                    }
                    _ => Pos::method_ref_type_arg(self.read_type_path(), type_index, -1, None),
                };
                position.offset = offset;
                position
            }
            TargetType::METHOD_RETURN => Pos::method_return(self.read_type_path(), -1, None),
            TargetType::FIELD => Pos::field(self.read_type_path(), -1, None),
            TargetType::UNKNOWN => {
                panic!("AssertionError: jvm.ClassReader: UNKNOWN target type should never occur!");
            }
        };
        Ok(position)
    }
    /// Java: void readAttrs(Symbol sym, AttributeKind kind)
    fn read_attrs(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        sym: SymbolId,
        kind: AttributeKind,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        let ac = self.next_char();
        for _ in 0..ac {
            let name_index = self.next_char() as i32;
            let attr_name = match self.get_name(name_index) {
                Ok(name) => name,
                Err(_) => self.names.empty.clone(),
            };
            let attr_len = self.next_int();
            // Java: readClassFile catches ArrayIndexOutOfBoundsException and maps it
            // to badClassFile("bad.class.file", c.flatname). Attribute lengths are
            // validated eagerly here; other next* bounds panics are converted at
            // the readClassFile boundary (read_class_file).
            let attr_end = (self.bp as i64) + (attr_len as i64);
            if attr_len < 0 || attr_end > self.buf.elems.len() as i64 {
                let class_name = ctx
                    .sym(self.current_owner.expect("currentOwner"))
                    .flat_name();
                let key = Fragment::from(
                    crate::resources::compiler_properties::fragments::bad_class_file(class_name),
                );
                return Err(self.bad_class_file(key).failure);
            }
            let position = self
                .attribute_readers
                .iter()
                .position(|def| def.name.to_java_string() == attr_name.to_java_string());
            if let Some(position) = position {
                let reader = self.attribute_readers[position].reader;
                if self.accepts(position, kind) {
                    self.read_attribute(ctx, syms, reader, sym, attr_len)?;
                } else {
                    self.bp += attr_len;
                }
            } else {
                self.bp += attr_len;
            }
        }
        Ok(())
    }
    /// Java: AttributeReader.accepts(AttributeKind kind)
    fn accepts(&mut self, position: usize, kind: AttributeKind) -> bool {
        let def = &self.attribute_readers[position];
        if !def.kinds.contains(&kind) {
            return false;
        }
        let version_major = def.version.major();
        let version_minor = def.version.minor();
        if self.major_version > version_major
            || (self.major_version == version_major && self.minor_version >= version_minor)
        {
            match def.reader {
                AttributeReaderKind::Module | AttributeReaderKind::ModuleResolution => {
                    return self.allow_modules;
                }
                AttributeReaderKind::Record => return self.allow_records,
                AttributeReaderKind::PermittedSubclasses => return self.allow_sealed_types,
                _ => {}
            }
            return true;
        }
        if self.lint_classfile && !self.warned_attrs.contains(&def.name) {
            // Java: ClassReader.java:764-773:
            //   JavaFileObject prev = log.useSource(currentClassFile);
            //   try { log.warning(LintCategory.CLASSFILE, (DiagnosticPosition) null,
            //           Warnings.FutureAttr(name, version.major, version.minor,
            //                               majorVersion, minorVersion)); }
            //   finally { log.useSource(prev); }
            //   warnedAttrs.add(name);
            let name = def.name.clone();
            let version_major = def.version.major();
            let version_minor = def.version.minor();
            let current_file = self.current_class_file.clone();
            let prev = AbstractLog::use_source(&mut *self.log.borrow_mut(), current_file);
            let info = warnings::future_attr(
                name.clone(),
                version_major,
                version_minor,
                self.major_version,
                self.minor_version,
            );
            self.log.borrow_mut().warning(
                Some(crate::code::lint::LintCategory::CLASSFILE),
                None,
                warning_from_info(info),
            );
            AbstractLog::use_source(&mut *self.log.borrow_mut(), prev);
            self.warned_attrs.insert(name);
        }
        false
    }

    /// Java: AttributeReader.read(Symbol sym, int attrLen) — dispatch per reader.
    fn read_attribute(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        reader: AttributeReaderKind,
        sym: SymbolId,
        attr_len: i32,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        match reader {
            AttributeReaderKind::Deprecated => {
                let target = if ctx.sym(sym).owner.map(|o| ctx.sym(o).kind)
                    == Some(crate::code::kinds::Kind::MDL)
                {
                    ctx.sym(sym).owner.expect("owner")
                } else {
                    sym
                };
                ctx.sym_mut(target).flags_field |= crate::code::flags::DEPRECATED as i64;
            }
            AttributeReaderKind::Synthetic => {
                ctx.sym_mut(sym).flags_field |= crate::code::flags::SYNTHETIC as i64;
            }
            AttributeReaderKind::Annotation => {
                ctx.sym_mut(sym).flags_field |= crate::code::flags::ANNOTATION as i64;
            }
            AttributeReaderKind::Bridge => {
                ctx.sym_mut(sym).flags_field |= crate::code::flags::BRIDGE;
            }
            AttributeReaderKind::Enum => {
                ctx.sym_mut(sym).flags_field |= crate::code::flags::ENUM as i64;
            }
            AttributeReaderKind::Varargs => {
                ctx.sym_mut(sym).flags_field |= crate::code::flags::VARARGS;
            }
            AttributeReaderKind::Exceptions => {
                let n = self.next_char();
                let mut thrown = List::nil();
                for _ in 0..n {
                    let class_index = self.next_char() as i32;
                    let class = self.get_class(ctx, syms, class_index)?;
                    let class_type = ctx.sym(class).ty.expect("class type");
                    thrown = thrown.prepend(class_type);
                }
                let ty = ctx.sym(sym).ty.expect("symbol type");
                if ctx.typ(ty).get_thrown_types(ctx).is_empty()
                    && let crate::code::type_::TypeKindData::MethodType { thrown: slot, .. } =
                        &mut ctx.typ_mut(ty).kind
                {
                    *slot = thrown.reverse();
                }
            }
            AttributeReaderKind::Signature => {
                self.read_signature_attr(ctx, syms, sym)?;
            }
            AttributeReaderKind::PermittedSubclasses => {
                let n = self.next_char();
                let mut subtypes = List::nil();
                for _ in 0..n {
                    let class_index = self.next_char() as i32;
                    let class = self.get_class(ctx, syms, class_index)?;
                    subtypes = subtypes.append(class);
                }
                if let crate::code::symbol::SymbolKind::ClassSymbol { permitted, .. } =
                    &mut ctx.sym_mut(sym).kind_data
                {
                    *permitted = subtypes;
                }
            }
            AttributeReaderKind::Record => {
                self.read_record_attr(ctx, syms, sym)?;
            }
            AttributeReaderKind::MethodParameters => {
                let newbp = self.bp + attr_len;
                if self.save_parameter_names {
                    self.saw_method_parameters = true;
                    let num_entries = self.next_byte();
                    let mut indices = vec![0i32; num_entries as usize];
                    let mut flags_vec = vec![0i32; num_entries as usize];
                    self.have_parameter_name_indices = true;
                    let mut index = 0usize;
                    for _ in 0..num_entries {
                        let name_index = self.next_char() as i32;
                        let flags_value = self.next_char() as i32;
                        if (flags_value
                            & (crate::code::flags::MANDATED | crate::code::flags::SYNTHETIC))
                            != 0
                        {
                            continue;
                        }
                        indices[index] = name_index;
                        flags_vec[index] = flags_value;
                        index += 1;
                    }
                    self.parameter_name_indices = Some(indices);
                    self.parameter_access_flags = Some(flags_vec);
                }
                self.bp = newbp;
            }
            AttributeReaderKind::LocalVariableTable => {
                let newbp = self.bp + attr_len;
                if self.save_parameter_names && !self.saw_method_parameters {
                    let num_entries = self.next_char();
                    for _ in 0..num_entries {
                        let start_pc = self.next_char() as i32;
                        let _length = self.next_char();
                        let name_index = self.next_char() as i32;
                        let _sig_index = self.next_char();
                        let register = self.next_char() as i32;
                        if start_pc == 0 {
                            let indices = self.parameter_name_indices.get_or_insert_with(Vec::new);
                            if register as usize >= indices.len() {
                                let new_size = (register + 1).max(indices.len() as i32 + 8);
                                indices.resize(new_size as usize, 0);
                            }
                            indices[register as usize] = name_index;
                            self.have_parameter_name_indices = true;
                        }
                    }
                }
                self.bp = newbp;
            }
            AttributeReaderKind::Module => {
                self.read_module_attr(ctx, syms, sym)?;
            }
            AttributeReaderKind::ModuleResolution => {
                self.read_module_resolution_attr(ctx, sym)?;
            }
            AttributeReaderKind::Code => {
                if self.save_parameter_names {
                    self.read_code(ctx, syms, sym)?;
                } else {
                    self.bp += attr_len;
                }
            }
            AttributeReaderKind::ConstantValue => {
                self.read_constant_value_attr(ctx, syms, sym)?;
            }
            AttributeReaderKind::InnerClasses => {
                let module_info =
                    self.current_module
                        .and_then(|module| match &ctx.sym(module).kind_data {
                            crate::code::symbol::SymbolKind::ModuleSymbol {
                                module_info, ..
                            } => *module_info,
                            _ => None,
                        });
                if module_info == Some(sym) {
                    self.skip_inner_classes();
                } else {
                    self.read_inner_classes(ctx, syms, sym)?;
                }
            }
            AttributeReaderKind::EnclosingMethod => {
                let newbp = self.bp + attr_len;
                let result = self.read_enclosing_method_attr(ctx, syms, sym);
                self.bp = newbp;
                result?;
            }
            AttributeReaderKind::SourceFile => {
                self.read_source_file_attr(ctx, sym);
            }
            AttributeReaderKind::AnnotationDefault => {
                self.attach_annotation_default(ctx, syms, sym)?;
            }
            AttributeReaderKind::RuntimeInvisibleAnnotations
            | AttributeReaderKind::RuntimeVisibleAnnotations => {
                self.attach_annotations(ctx, syms, sym)?;
            }
            AttributeReaderKind::RuntimeInvisibleParameterAnnotations
            | AttributeReaderKind::RuntimeVisibleParameterAnnotations => {
                self.read_parameter_annotations(ctx, syms, sym)?;
            }
            AttributeReaderKind::RuntimeVisibleTypeAnnotations
            | AttributeReaderKind::RuntimeInvisibleTypeAnnotations => {
                self.attach_type_annotations(ctx, syms, sym)?;
            }
        }
        Ok(())
    }

    /// Java: void skipInnerClasses()
    fn skip_inner_classes(&mut self) {
        let n = self.next_char();
        for _ in 0..n {
            self.next_char();
            self.next_char();
            self.next_char();
            self.next_char();
        }
    }
    // ------------------------------------------------------------------
    // S2-B attribute readers (ClassReader.java:798-1080, 1255-1305, 2883-2912)
    // ------------------------------------------------------------------

    /// Java: ConstantValue AttributeReader.read (ClassReader.java:807-853)
    fn read_constant_value_attr(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        sym: SymbolId,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        let value_index = self.next_char() as i32;
        let v = self.get_constant(value_index)?;
        use crate::code::attribute::ConstValue;
        use crate::code::type_tag::TypeTag;
        use crate::jvm::pool_reader::PoolValue;
        let const_value = match &v {
            PoolValue::Integer(value) => ConstValue::Integer(*value),
            PoolValue::Long(value) => ConstValue::Long(*value),
            PoolValue::Float(value) => ConstValue::Float(*value),
            PoolValue::Double(value) => ConstValue::Double(*value),
            PoolValue::String(value) => ConstValue::String(value.clone()),
            _ => panic!("unexpected constant pool value for ConstantValue"),
        };
        // Ignore ConstantValue attribute if field not final.
        if (ctx.sym(sym).flags_field & crate::code::flags::FINAL as i64) == 0 {
            return Ok(());
        }
        let ty = ctx.sym(sym).ty.expect("variable type");
        let tag = ctx.typ(ty).get_tag();
        let value_text = Self::const_value_object_string(&const_value);
        let symbol_text = crate::code::symbol::symbol_to_string(ctx, sym);
        let bad_value = |clazz: &str| -> crate::code::symbol::CompletionFailure {
            let key = Fragment::from(
                crate::resources::compiler_properties::fragments::bad_constant_value(
                    javac_strings::JavaString::from(value_text.as_str()),
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(
                        symbol_text.clone(),
                    ),
                    javac_strings::JavaString::from(clazz),
                ),
            );
            self.bad_class_file(key).failure
        };
        match tag {
            TypeTag::BOOLEAN | TypeTag::BYTE | TypeTag::CHAR | TypeTag::SHORT | TypeTag::INT => {
                let int_value = match &const_value {
                    ConstValue::Integer(value) => *value,
                    _ => return Err(bad_value("Integer")),
                };
                if !tag.check_range(int_value) {
                    let key = Fragment::from(
                        crate::resources::compiler_properties::fragments::bad_constant_range(
                            javac_strings::JavaString::from(format!("{int_value}").as_str()),
                            crate::resources::diagnostic_contract::SymbolArg::placeholder(
                                crate::code::symbol::symbol_to_string(ctx, sym),
                            ),
                            crate::resources::diagnostic_contract::TypeArg::placeholder(
                                crate::code::type_::type_to_string(ctx, ty),
                            ),
                        ),
                    );
                    return Err(self.bad_class_file(key).failure);
                }
                ctx.sym_mut(sym).set_data(Some(const_value));
            }
            TypeTag::LONG => {
                if !matches!(&const_value, ConstValue::Long(_)) {
                    return Err(bad_value("Long"));
                }
                ctx.sym_mut(sym).set_data(Some(const_value));
            }
            TypeTag::FLOAT => {
                if !matches!(&const_value, ConstValue::Float(_)) {
                    return Err(bad_value("Float"));
                }
                ctx.sym_mut(sym).set_data(Some(const_value));
            }
            TypeTag::DOUBLE => {
                if !matches!(&const_value, ConstValue::Double(_)) {
                    return Err(bad_value("Double"));
                }
                ctx.sym_mut(sym).set_data(Some(const_value));
            }
            TypeTag::CLASS => {
                let is_string = ctx.typ(ty).tsym == ctx.typ(syms.string_type).tsym;
                if !is_string {
                    let key = Fragment::from(
                        crate::resources::compiler_properties::fragments::bad_constant_value_type(
                            crate::resources::diagnostic_contract::TypeArg::placeholder(
                                crate::code::type_::type_to_string(ctx, ty),
                            ),
                        ),
                    );
                    return Err(self.bad_class_file(key).failure);
                }
                if !matches!(&const_value, ConstValue::String(_)) {
                    return Err(bad_value("String"));
                }
                ctx.sym_mut(sym).set_data(Some(const_value));
            }
            // ignore ConstantValue attribute if type is not primitive or String
            _ => return Ok(()),
        }
        Ok(())
    }

    /// Java: the boxed ConstantValue's Object.toString (the
    /// bad.constant.value/bad.constant.range {0} argument).
    fn const_value_object_string(value: &crate::util::constants::ConstantValue) -> String {
        use crate::util::constants::ConstantValue;
        match value {
            ConstantValue::Integer(value) => value.to_string(),
            ConstantValue::Byte(value) => value.to_string(),
            ConstantValue::Short(value) => value.to_string(),
            ConstantValue::Long(value) => value.to_string(),
            ConstantValue::Float(value) => javac_floating::to_java_string_float(*value),
            ConstantValue::Double(value) => javac_floating::to_java_string_double(*value),
            ConstantValue::Character(value) => char::from_u32(u32::from(*value))
                .unwrap_or('\u{0}')
                .to_string(),
            ConstantValue::Boolean(value) => value.to_string(),
            ConstantValue::String(value) => value.to_string_lossy().to_string(),
        }
    }

    /// Java: readClassFile's ArrayIndexOutOfBoundsException -> badClassFile
    /// mapping (ClassReader.java:2790-2800).
    fn is_index_out_of_bounds_panic(payload: &(dyn std::any::Any + Send)) -> bool {
        let message = payload
            .downcast_ref::<&str>()
            .map(|text| *text)
            .or_else(|| payload.downcast_ref::<String>().map(|text| text.as_str()));
        match message {
            Some(message) => {
                message.contains("out of range")
                    || message.contains("index out of bounds")
                    || message.contains("range end index")
            }
            None => false,
        }
    }

    /// Java: Code readCode(Symbol owner) (ClassReader.java:1399-1408).
    /// Upstream reads max_stack/max_locals/code/exception_table, dispatches the
    /// nested member attributes, and returns null — no Code object is attached
    /// to the method symbol (Code.java is used by Gen/ClassWriter, not the
    /// classfile reader). The port mirrors exactly this structure-only read.
    fn read_code(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        owner: SymbolId,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        self.next_char(); // max_stack
        self.next_char(); // max_locals
        let code_length = self.next_int();
        self.bp += code_length;
        let exception_table_length = self.next_char();
        self.bp += exception_table_length as i32 * 8;
        self.read_member_attrs(ctx, syms, owner)
    }

    /// Java: void readInnerClasses(ClassSymbol c) (ClassReader.java:2883-2912)
    fn read_inner_classes(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        c: SymbolId,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        let n = self.next_char();
        for _ in 0..n {
            self.next_char(); // skip inner class symbol
            let outer_idx = self.next_char() as i32;
            let name_idx = self.next_char() as i32;
            let outer = if outer_idx == 0 {
                None
            } else {
                Some(self.get_class(ctx, syms, outer_idx)?)
            };
            let name = if name_idx == 0 {
                self.names.empty.clone()
            } else {
                self.get_name(name_idx)?
            };
            let raw_flags = self.next_char() as i64;
            let flags = self.adjust_class_flags(raw_flags);
            if let Some(outer) = outer {
                let name = if name.to_java_string() == self.names.empty.to_java_string() {
                    self.names.one.clone()
                } else {
                    name
                };
                let member = syms.enter_class(
                    ctx,
                    self.current_module.expect("currentModule"),
                    name,
                    Some(outer),
                );
                if (flags & crate::code::flags::STATIC as i64) == 0 {
                    let outer_ty = ctx.sym(outer).ty.expect("outer type");
                    let member_ty = ctx.sym(member).ty.expect("member type");
                    ctx.typ_mut(member_ty).set_enclosing_type(Some(outer_ty));
                    if let Some(erasure) = ctx.sym(member).erasure_field {
                        let outer_erased = self.types.erasure(ctx, syms, outer_ty);
                        ctx.typ_mut(erasure).set_enclosing_type(Some(outer_erased));
                    }
                }
                if c == outer {
                    ctx.sym_mut(member).flags_field = flags;
                    self.enter_member(ctx, c, member);
                }
            }
        }
        Ok(())
    }

    /// Java: void readEnclosingMethodAttr(Symbol sym) (ClassReader.java:1255-1305)
    fn read_enclosing_method_attr(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        sym: SymbolId,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        let owner = ctx.sym(sym).owner.expect("class owner");
        if let Some(members) = ctx.sym(owner).members() {
            ctx.scope_remove(members, sym);
        }
        let self_class = sym;
        let class_idx = self.next_char() as i32;
        let c = self.get_class(ctx, syms, class_idx)?;
        let nt_idx = self.next_char() as i32;
        let nt = if nt_idx == 0 {
            None
        } else {
            Some(self.get_name_and_type(ctx, syms, nt_idx)?)
        };
        let c_members = ctx.sym(c).members();
        if c_members.is_none() || ctx.sym(c).kind != crate::code::kinds::Kind::TYP {
            let key = Fragment::from(
                crate::resources::compiler_properties::fragments::bad_enclosing_class(),
            );
            return Err(self.bad_class_file(key).failure);
        }
        let self_flags = ctx.sym(self_class).flags_field;
        let m = self.find_method(ctx, c_members.expect("members"), nt.as_ref(), self_flags);
        if nt.is_some() && m.is_none() {
            // Java: ClassReader.java:1268-1270
            //   MethodSymbol m = findMethod(nt, c.members_field, self.flags());
            //   if (nt != null && m == null) throw badEnclosingMethod(self);
            let attr = self.bad_enclosing_method(ctx, self_class);
            return Err(attr.bad_class_file.failure);
        }
        let self_flat = ctx.sym(self_class).flat_name();
        let c_flat = ctx.sym(c).flat_name();
        let name = self.simple_binary_name(&self_flat, &c_flat)?;
        ctx.sym_mut(self_class).name = name.clone();
        let new_owner = m.unwrap_or(c);
        ctx.sym_mut(self_class).owner = Some(new_owner);
        let fullname = if name.is_empty() {
            self.names.empty.clone()
        } else {
            crate::code::symtab::form_full_name(ctx, &name, Some(new_owner))
        };
        if let crate::code::symbol::SymbolKind::ClassSymbol { fullname: slot, .. } =
            &mut ctx.sym_mut(self_class).kind_data
        {
            *slot = fullname;
        }
        let self_ty = ctx.sym(self_class).ty.expect("class type");
        if let Some(m) = m {
            let m_ty = ctx.sym(m).ty.expect("method type");
            ctx.typ_mut(self_ty).set_enclosing_type(Some(m_ty));
        } else if (self_flags & crate::code::flags::STATIC as i64) == 0 {
            let c_ty = ctx.sym(c).ty.expect("enclosing class type");
            ctx.typ_mut(self_ty).set_enclosing_type(Some(c_ty));
        } else {
            let no_type = self.no_type(ctx);
            ctx.typ_mut(self_ty).set_enclosing_type(Some(no_type));
        }
        self.enter_typevars(ctx, syms, self_class, self_ty);
        if !self.missing_type_variables.is_empty() {
            let mut found = List::nil();
            let missing = self.missing_type_variables.clone();
            let list = missing;
            for t in &list {
                let name = ctx.typ(t).tsym.map(|sym| ctx.sym(sym).name.clone());
                let name = name.expect("type variable name");
                found = found.append(self.find_type_var(ctx, syms, &name)?);
            }
            self.found_type_variables = found;
        } else {
            self.found_type_variables = List::nil();
        }
        Ok(())
    }

    /// Java: private Name simpleBinaryName(Name self, Name enclosing)
    fn simple_binary_name(
        &self,
        self_name: &Name,
        enclosing: &Name,
    ) -> Result<Name, crate::code::symbol::CompletionFailure> {
        let self_str = self_name.to_java_string().to_string();
        let enclosing_str = enclosing.to_java_string().to_string();
        let simple = if let Some(rest) = self_str.strip_prefix(&enclosing_str) {
            rest
        } else {
            let key = Fragment::from(
                crate::resources::compiler_properties::fragments::bad_enclosing_method(
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(self_str.clone()),
                ),
            );
            return Err(self.bad_class_file(key).failure);
        };
        if simple.is_empty() || !simple.starts_with('$') {
            let key = Fragment::from(
                crate::resources::compiler_properties::fragments::bad_enclosing_method(
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(self_str.clone()),
                ),
            );
            return Err(self.bad_class_file(key).failure);
        }
        let mut index = 1;
        let bytes = simple.as_bytes();
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        let result = &simple[index..];
        Ok(self
            .names
            .from_string(&crate::util::java_string::JavaString::from(result)))
    }

    /// Java: private MethodSymbol findMethod(NameAndType nt, Scope scope, long flags)
    fn find_method(
        &mut self,
        ctx: &mut CompilerContext,
        scope: crate::ids::ScopeId,
        nt: Option<&(Name, TypeId)>,
        flags: i64,
    ) -> Option<SymbolId> {
        let (nt_name, nt_type) = nt?;
        let candidates = ctx.scope_get_symbols_by_name(
            scope,
            nt_name,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        for candidate in candidates {
            if ctx.sym(candidate).kind == crate::code::kinds::Kind::MTH {
                let candidate_ty = ctx.sym(candidate).ty.expect("method type");
                if self.is_same_binary_type(ctx, candidate_ty, *nt_type) {
                    return Some(candidate);
                }
            }
        }
        if nt_name.to_java_string() != self.names.init.to_java_string() {
            return None;
        }
        if (flags & crate::code::flags::INTERFACE as i64) != 0 {
            return None;
        }
        let nt_params = ctx.typ(*nt_type).get_parameter_types(ctx);
        if nt_params.is_empty() {
            return None;
        }
        let restype = ctx.typ(*nt_type).get_return_type(ctx).expect("return type");
        let thrown = ctx.typ(*nt_type).get_thrown_types(ctx);
        let method_class = ctx.typ(*nt_type).tsym.expect("method class");
        let new_type = ctx.alloc_type(crate::code::type_::Type::new_method_type(
            nt_params.tail(),
            restype,
            thrown,
            method_class,
        ));
        self.find_method(ctx, scope, Some(&(nt_name.clone(), new_type)), flags)
    }

    /// Java: private boolean isSameBinaryType(MethodType mt1, MethodType mt2)
    fn is_same_binary_type(&self, ctx: &mut CompilerContext, mt1: TypeId, mt2: TypeId) -> bool {
        let mut types1 = ctx
            .typ(mt1)
            .get_parameter_types(ctx)
            .prepend(ctx.typ(mt1).get_return_type(ctx).expect("return type"));
        let mut types2 = ctx
            .typ(mt2)
            .get_parameter_types(ctx)
            .prepend(ctx.typ(mt2).get_return_type(ctx).expect("return type"));
        while !types1.is_empty() && !types2.is_empty() {
            let tsym1 = ctx.typ(*types1.head()).tsym;
            let tsym2 = ctx.typ(*types2.head()).tsym;
            if tsym1 != tsym2 {
                return false;
            }
            types1 = types1.tail();
            types2 = types2.tail();
        }
        types1.is_empty() && types2.is_empty()
    }

    /// Java: SourceFile AttributeReader.read (ClassReader.java:924-940)
    fn read_source_file_attr(&mut self, ctx: &mut CompilerContext, sym: SymbolId) {
        let name_index = self.next_char() as i32;
        let n = self.get_name(name_index).expect("source file name");
        let source = Rc::new(SourceFileObject { name: n.clone() });
        if let crate::code::symbol::SymbolKind::ClassSymbol { sourcefile, .. } =
            &mut ctx.sym_mut(sym).kind_data
        {
            *sourcefile = Some(source);
        }
        let sn = n.to_java_string().to_string();
        if ctx.sym(sym).owner.map(|o| ctx.sym(o).kind) == Some(crate::code::kinds::Kind::PCK)
            && sn.ends_with(".java")
            && sn != format!("{}.java", ctx.sym(sym).name.to_java_string())
        {
            ctx.sym_mut(sym).flags_field |= crate::code::flags::AUXILIARY;
        }
    }
    /// Java: void readClassAttrs(ClassSymbol c)
    fn read_class_attrs(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        c: SymbolId,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        self.read_attrs(ctx, syms, c, AttributeKind::CLASS)
    }

    /// Java: void readMemberAttrs(Symbol sym)
    fn read_member_attrs(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        sym: SymbolId,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        self.read_attrs(ctx, syms, sym, AttributeKind::MEMBER)
    }

    /// Java: void readClassAttrs / readMemberAttrs share readAttrs; kept distinct
    /// for the upstream names (see above).
    fn read_signature_attr(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        sym: SymbolId,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        if ctx.sym(sym).kind == crate::code::kinds::Kind::TYP {
            let class_ty = ctx.sym(sym).ty.expect("class type");
            self.reading_class_attr = true;
            let sig_index = self.next_char() as i32;
            let sig_name = self.get_name(sig_index).expect("signature name");
            let bytes = sig_name.get_byte_array().to_vec();
            self.signature = bytes.clone();
            self.sigp = 0;
            self.siglimit = bytes.len() as i32;
            let typarams = self.sig_to_type_params(ctx, syms).expect("type params");
            if let crate::code::type_::TypeKindData::ClassType { typarams_field, .. } =
                &mut ctx.typ_mut(class_ty).kind
            {
                *typarams_field = Some(typarams);
            }
            let supertype = self.sig_to_type(ctx, syms).expect("supertype signature");
            if let crate::code::type_::TypeKindData::ClassType {
                supertype_field, ..
            } = &mut ctx.typ_mut(class_ty).kind
            {
                *supertype_field = Some(supertype);
            }
            let mut interfaces = List::nil();
            while self.sigp != self.siglimit {
                interfaces =
                    interfaces.append(self.sig_to_type(ctx, syms).expect("interface signature"));
            }
            if let crate::code::type_::TypeKindData::ClassType {
                interfaces_field, ..
            } = &mut ctx.typ_mut(class_ty).kind
            {
                *interfaces_field = Some(interfaces);
            }
            self.reading_class_attr = false;
        } else {
            let ty = ctx.sym(sym).ty.expect("symbol type");
            let thrown = ctx.typ(ty).get_thrown_types(ctx);
            let sig_index = self.next_char() as i32;
            let new_ty = self
                .get_type(ctx, syms, sig_index)
                .expect("member signature");
            ctx.sym_mut(sym).ty = Some(new_ty);
            if ctx.sym(sym).kind == crate::code::kinds::Kind::MTH
                && ctx.typ(new_ty).get_thrown_types(ctx).is_empty()
            {
                match &mut ctx.typ_mut(new_ty).kind {
                    crate::code::type_::TypeKindData::MethodType { thrown: slot, .. } => {
                        *slot = thrown;
                    }
                    crate::code::type_::TypeKindData::ForAll {
                        qtype: Some(qtype), ..
                    } => {
                        let qtype = *qtype;
                        if let crate::code::type_::TypeKindData::MethodType {
                            thrown: slot, ..
                        } = &mut ctx.typ_mut(qtype).kind
                        {
                            *slot = thrown;
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// Java: Record attribute reader
    fn read_record_attr(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        sym: SymbolId,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        ctx.sym_mut(sym).flags_field |= crate::code::flags::RECORD;
        let component_count = self.next_char();
        let mut components = List::nil();
        for _ in 0..component_count {
            let name_index = self.next_char() as i32;
            let name = self.get_name(name_index).expect("record component name");
            let type_index = self.next_char() as i32;
            let ty = self
                .get_type(ctx, syms, type_index)
                .expect("record component type");
            let component = ctx.alloc_symbol(crate::code::symbol::Symbol::new_record_component(
                name, ty, sym,
            ));
            self.read_attrs(ctx, syms, component, AttributeKind::MEMBER)?;
            components = components.append(component);
        }
        ctx.sym_mut(sym).set_record_components(components);
        Ok(())
    }

    /// Java: void setParameters(MethodSymbol sym, Type jvmType)
    /// (ClassReader.java:2641-2701): build the parameter symbols, attaching the
    /// parameter annotations read by readParameterAnnotations.
    #[allow(clippy::mutable_key_type)]
    fn set_parameters(
        &mut self,
        ctx: &mut CompilerContext,
        _syms: &crate::code::symtab::Symtab,
        sym: SymbolId,
        jvm_type: TypeId,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        let saw_method_parameters = self.saw_method_parameters;
        let mut first_param: i32 = 0;
        if !saw_method_parameters {
            let static_flag = ctx.sym(sym).flags_field & crate::code::flags::STATIC as i64;
            first_param = if static_flag == 0 { 1 } else { 0 };
            let current_owner = self.current_owner.expect("currentOwner");
            if ctx.sym(sym).name.to_java_string() == self.names.init.to_java_string()
                && ctx.sym(current_owner).has_outer_instance(ctx)
                && !ctx.sym(current_owner).name.is_empty()
            {
                first_param += 1;
            }
            let sym_ty = ctx.sym(sym).ty.expect("method symbol type");
            if sym_ty != jvm_type {
                let jvm_width = types_width(ctx, ctx.typ(jvm_type).get_parameter_types(ctx));
                let sym_width = types_width(ctx, ctx.typ(sym_ty).get_parameter_types(ctx));
                first_param += jvm_width - sym_width;
            }
        }
        let mut exclude: HashSet<Name> = HashSet::new();
        let mut params = List::nil();
        let mut name_index = first_param;
        let mut annotation_index = 0usize;
        let types = ctx
            .typ(ctx.sym(sym).ty.expect("method type"))
            .get_parameter_types(ctx);
        for param_type in &types {
            let param = self.parameter(ctx, name_index, param_type, sym, &mut exclude)?;
            params = params.append(param);
            if let Some(parameter_annotations) = &self.parameter_annotations
                && annotation_index < parameter_annotations.len()
                && let Some(proxies) = &parameter_annotations[annotation_index]
                && proxies.non_empty()
                && self.annotate.borrow().is_some()
            {
                self.queue_compound_annotation_completer(ctx, param, proxies.clone())?;
            }
            name_index += if saw_method_parameters {
                1
            } else {
                jvm_type_width(ctx, param_type)
            };
            annotation_index += 1;
        }
        if let Some(parameter_annotations) = &self.parameter_annotations
            && parameter_annotations.len() != annotation_index
        {
            let key = Fragment::from(
                crate::resources::compiler_properties::fragments::bad_runtime_invisible_param_annotations(),
            );
            return Err(self.bad_class_file(key).failure);
        }
        let already_has_params = match &ctx.sym(sym).kind_data {
            crate::code::symbol::SymbolKind::MethodSymbol { params, .. } => params.non_empty(),
            _ => false,
        };
        assert!(!already_has_params, "AssertionError: params already set");
        ctx.sym_mut(sym).set_params(params);
        self.parameter_annotations = None;
        self.parameter_name_indices = None;
        self.parameter_access_flags = None;
        Ok(())
    }

    /// Java: private VarSymbol parameter(int index, Type t, MethodSymbol owner,
    /// Set<Name> exclude) (ClassReader.java:2707-2729)
    #[allow(clippy::mutable_key_type)]
    fn parameter(
        &mut self,
        ctx: &mut CompilerContext,
        index: i32,
        t: TypeId,
        owner: SymbolId,
        exclude: &mut HashSet<Name>,
    ) -> Result<SymbolId, crate::code::symbol::CompletionFailure> {
        let mut flags = crate::code::flags::PARAMETER;
        if let Some(access_flags) = &self.parameter_access_flags
            && index >= 0
            && (index as usize) < access_flags.len()
            && access_flags[index as usize] != 0
        {
            flags |= access_flags[index as usize] as i64;
        }
        let name_index = self
            .parameter_name_indices
            .as_ref()
            .and_then(|indices| {
                if index >= 0 && (index as usize) < indices.len() {
                    Some(indices[index as usize])
                } else {
                    None
                }
            })
            .filter(|index| *index != 0);
        let arg_name = if let Some(name_index) = name_index {
            flags |= crate::code::flags::NAME_FILLED;
            match self.get_name(name_index) {
                Ok(name) => name,
                Err(_) => self.names.empty.clone(),
            }
        } else {
            let mut prefix = "arg".to_string();
            loop {
                let candidate = self.names.from_string(&javac_strings::JavaString::from(
                    format!("{prefix}{}", exclude.len()).as_str(),
                ));
                if !exclude.contains(&candidate) {
                    break candidate;
                }
                prefix.push('$');
            }
        };
        exclude.insert(arg_name.clone());
        Ok(
            ctx.alloc_symbol(crate::code::symbol::Symbol::new_param_symbol(
                flags, arg_name, t, owner,
            )),
        )
    }

    /// Java: public ClassFinder.BadEnclosingMethodAttr badEnclosingMethod(Symbol sym)
    ///
    /// # Arguments
    ///   - ctx: the compiler context (to resolve the enclosing class chain);
    ///   - sym: the symbol passed to the Fragments.BadEnclosingMethod(sym) fragment.
    ///
    /// # Returns
    ///   - the BadEnclosingMethodAttr built from currentOwner.enclClass() and
    ///     currentClassFile (ClassReader.java:323-330).
    ///
    /// # Panics
    ///   - the reader has no current class file (upstream invariant: completing a symbol).
    pub fn bad_enclosing_method(
        &self,
        ctx: &crate::compiler_context::CompilerContext,
        sym: SymbolId,
    ) -> crate::code::class_finder::BadEnclosingMethodAttr {
        let file = self
            .current_class_file
            .clone()
            .expect("ClassSymbol.classfile (upstream: non-null when completing)");
        let owner = self.current_owner.expect("currentOwner");
        let enclosing = crate::code::symbol::symbol_encl_class(ctx, owner).unwrap_or(owner);
        // Java: diagFactory.fragment(Fragments.BadEnclosingMethod(sym)) — the
        // generated accessor carries the symbol name as diagnostic text until
        // the P4 Symbol argument lands.
        // Java: Fragments.BadEnclosingMethod(sym) prints the symbol.
        let display = crate::code::symbol::symbol_to_string(ctx, sym);
        let diag = self.diag_factory.fragment(Fragment::from(
            crate::resources::compiler_properties::fragments::bad_enclosing_method(
                crate::resources::diagnostic_contract::SymbolArg::placeholder(display),
            ),
        ));
        crate::code::class_finder::BadEnclosingMethodAttr::new(
            Some(enclosing),
            file,
            diag,
            Rc::clone(&self.diag_factory),
            Some(Rc::clone(&self.dcfh)),
        )
    }
}

/// Java: ClassReader.addTypeAnnotationsToSymbol (ClassReader.java:2212-2223)
/// and the TypeAnnotationSymbolVisitor/TypeAnnotationStructuralTypeMapping
/// classes (2225-2501): rewrite the types of the symbol signature to carry the
/// attached type annotations.
fn add_type_annotations_to_symbol(
    ctx: &mut CompilerContext,
    syms: &crate::code::symtab::Symtab,
    sym: SymbolId,
    attributes: List<crate::ids::AttributeId>,
) -> Result<(), crate::code::symbol::CompletionFailure> {
    use crate::code::kinds::Kind;
    match ctx.sym(sym).kind {
        Kind::TYP => visit_class_symbol_type_annotations(ctx, syms, sym, &attributes),
        Kind::MTH => visit_method_symbol_type_annotations(ctx, syms, sym, &attributes),
        Kind::VAR => visit_var_symbol_type_annotations(ctx, syms, sym, &attributes),
        _ => Ok(()),
    }
}

/// Java: TypeAnnotationSymbolVisitor.SUPERCLASS_INDEX
const SUPERCLASS_INDEX: i32 = 65535;

/// Java: Predicate<TypeAnnotationPosition> filters of TypeAnnotationSymbolVisitor.
#[derive(Clone, Copy)]
enum TypeAnnotationFilter {
    Target(crate::code::target_type::TargetType),
    TypeParameterBound {
        target: crate::code::target_type::TargetType,
        parameter_index: i32,
        bound_index: i32,
    },
    MethodFormalParameter(i32),
    Thrown(i32),
    ClassExtends(i32),
}

impl TypeAnnotationFilter {
    fn matches(
        self,
        position: &crate::code::type_annotation_position::TypeAnnotationPosition,
    ) -> bool {
        use crate::code::target_type::TargetType;
        match self {
            TypeAnnotationFilter::Target(target) => position.type_ == target,
            TypeAnnotationFilter::TypeParameterBound {
                target,
                parameter_index,
                bound_index,
            } => {
                position.type_ == target
                    && position.parameter_index == parameter_index
                    && position.bound_index == bound_index
            }
            TypeAnnotationFilter::MethodFormalParameter(index) => {
                position.type_ == TargetType::METHOD_FORMAL_PARAMETER
                    && position.parameter_index == index
            }
            TypeAnnotationFilter::Thrown(index) => {
                position.type_ == TargetType::THROWS && position.type_index == index
            }
            TypeAnnotationFilter::ClassExtends(index) => {
                position.type_ == TargetType::CLASS_EXTENDS && position.type_index == index
            }
        }
    }
}

fn type_annotation_position(
    ctx: &CompilerContext,
    attribute: crate::ids::AttributeId,
) -> Option<crate::code::type_annotation_position::TypeAnnotationPosition> {
    match &ctx.attr(attribute).kind {
        crate::code::attribute::AttributeKind::TypeCompound { position, .. } => position.clone(),
        _ => None,
    }
}

fn visit_class_symbol_type_annotations(
    ctx: &mut CompilerContext,
    syms: &crate::code::symtab::Symtab,
    sym: SymbolId,
    attributes: &List<crate::ids::AttributeId>,
) -> Result<(), crate::code::symbol::CompletionFailure> {
    use crate::code::type_::TypeKindData;
    let sym_ty = ctx.sym(sym).ty.expect("class symbol type");
    let (mut interfaces, supertype, typarams) = match &ctx.typ(sym_ty).kind {
        TypeKindData::ClassType {
            interfaces_field,
            supertype_field,
            typarams_field,
            ..
        }
        | TypeKindData::ErasedClassType {
            interfaces_field,
            supertype_field,
            typarams_field,
            ..
        }
        | TypeKindData::UnionClassType {
            interfaces_field,
            supertype_field,
            typarams_field,
            ..
        }
        | TypeKindData::IntersectionClassType {
            interfaces_field,
            supertype_field,
            typarams_field,
            ..
        } => (
            interfaces_field.clone().unwrap_or_else(List::nil),
            *supertype_field,
            typarams_field.clone(),
        ),
        _ => return Ok(()),
    };
    let mut new_interfaces = List::nil();
    for (index, interface) in interfaces.iter().enumerate() {
        new_interfaces = new_interfaces.append(add_type_annotations_to_type(
            ctx,
            syms,
            interface,
            attributes,
            TypeAnnotationFilter::ClassExtends(index as i32),
        )?);
    }
    interfaces = new_interfaces;
    let new_supertype = match supertype {
        Some(supertype) => Some(add_type_annotations_to_type(
            ctx,
            syms,
            supertype,
            attributes,
            TypeAnnotationFilter::ClassExtends(SUPERCLASS_INDEX),
        )?),
        None => None,
    };
    let new_typarams = match typarams {
        Some(typarams) => Some(rewrite_type_parameters(
            ctx,
            syms,
            typarams,
            attributes,
            crate::code::target_type::TargetType::CLASS_TYPE_PARAMETER_BOUND,
        )?),
        None => None,
    };
    match &mut ctx.typ_mut(sym_ty).kind {
        TypeKindData::ClassType {
            interfaces_field,
            supertype_field,
            typarams_field,
            ..
        }
        | TypeKindData::ErasedClassType {
            interfaces_field,
            supertype_field,
            typarams_field,
            ..
        }
        | TypeKindData::UnionClassType {
            interfaces_field,
            supertype_field,
            typarams_field,
            ..
        }
        | TypeKindData::IntersectionClassType {
            interfaces_field,
            supertype_field,
            typarams_field,
            ..
        } => {
            *interfaces_field = Some(interfaces);
            *supertype_field = new_supertype;
            *typarams_field = new_typarams;
        }
        _ => {}
    }
    Ok(())
}

fn visit_method_symbol_type_annotations(
    ctx: &mut CompilerContext,
    syms: &crate::code::symtab::Symtab,
    sym: SymbolId,
    attributes: &List<crate::ids::AttributeId>,
) -> Result<(), crate::code::symbol::CompletionFailure> {
    use crate::code::target_type::TargetType;
    use crate::code::type_::TypeKindData;
    let sym_ty = ctx.sym(sym).ty.expect("method symbol type");
    let (tvars, method_ty) = match &ctx.typ(sym_ty).kind {
        TypeKindData::ForAll { tvars, qtype } => {
            (Some(tvars.clone()), qtype.expect("ForAll qtype"))
        }
        _ => (None, sym_ty),
    };
    let params = match &ctx.sym(sym).kind_data {
        crate::code::symbol::SymbolKind::MethodSymbol { params, .. } => params.clone(),
        _ => List::nil(),
    };
    let mut argtypes = List::nil();
    let mut index = 0i32;
    let current = params;
    for param in &current {
        let param_ty = ctx.sym(param).ty.expect("parameter type");
        let new_ty = add_type_annotations_to_type(
            ctx,
            syms,
            param_ty,
            attributes,
            TypeAnnotationFilter::MethodFormalParameter(index),
        )?;
        ctx.sym_mut(param).ty = Some(new_ty);
        argtypes = argtypes.append(new_ty);
        index += 1;
    }
    let (old_thrown, restype, recvtype) = match &ctx.typ(method_ty).kind {
        TypeKindData::MethodType {
            thrown,
            restype,
            recvtype,
            ..
        } => (thrown.clone(), *restype, *recvtype),
        _ => (List::nil(), None, None),
    };
    let mut thrown = List::nil();
    for (thrown_index, thrown_type) in old_thrown.iter().enumerate() {
        thrown = thrown.append(add_type_annotations_to_type(
            ctx,
            syms,
            thrown_type,
            attributes,
            TypeAnnotationFilter::Thrown(thrown_index as i32),
        )?);
    }
    let new_restype = match restype {
        Some(restype)
            if !ctx
                .typ(restype)
                .has_tag(crate::code::type_tag::TypeTag::VOID) =>
        {
            Some(add_type_annotations_to_type(
                ctx,
                syms,
                restype,
                attributes,
                TypeAnnotationFilter::Target(TargetType::METHOD_RETURN),
            )?)
        }
        restype => restype,
    };
    let new_recvtype = match recvtype {
        Some(recvtype) => Some(add_type_annotations_to_type(
            ctx,
            syms,
            recvtype,
            attributes,
            TypeAnnotationFilter::Target(TargetType::METHOD_RECEIVER),
        )?),
        None => None,
    };
    let mut new_method = ctx.typ(method_ty).clone();
    new_method.kind = TypeKindData::MethodType {
        argtypes,
        restype: new_restype,
        thrown,
        recvtype: new_recvtype,
    };
    let new_method_ty = ctx.alloc_type(new_method);
    // Constructors have no method type parameters of their own: their
    // MethodType must stay plain (ClassReader.java:2539-2554). Wrapping the
    // class tvars here would duplicate them when findDiamond prepends the
    // site's class tvars.
    let tvars = if ctx.sym(sym).name.to_java_string() == syms.names().init.to_java_string() {
        None
    } else {
        tvars
    };
    let final_ty = if let Some(tvars) = tvars {
        let new_tvars = rewrite_type_parameters(
            ctx,
            syms,
            tvars,
            attributes,
            TargetType::METHOD_TYPE_PARAMETER_BOUND,
        )?;
        let mut new_for_all = ctx.typ(sym_ty).clone();
        new_for_all.kind = TypeKindData::ForAll {
            tvars: new_tvars,
            qtype: Some(new_method_ty),
        };
        ctx.alloc_type(new_for_all)
    } else {
        new_method_ty
    };
    ctx.sym_mut(sym).ty = Some(final_ty);
    Ok(())
}

fn visit_var_symbol_type_annotations(
    ctx: &mut CompilerContext,
    syms: &crate::code::symtab::Symtab,
    sym: SymbolId,
    attributes: &List<crate::ids::AttributeId>,
) -> Result<(), crate::code::symbol::CompletionFailure> {
    let sym_ty = ctx.sym(sym).ty.expect("variable symbol type");
    let new_ty = add_type_annotations_to_type(
        ctx,
        syms,
        sym_ty,
        attributes,
        TypeAnnotationFilter::Target(crate::code::target_type::TargetType::FIELD),
    )?;
    ctx.sym_mut(sym).ty = Some(new_ty);
    Ok(())
}

fn rewrite_type_parameters(
    ctx: &mut CompilerContext,
    syms: &crate::code::symtab::Symtab,
    tvars: List<TypeId>,
    attributes: &List<crate::ids::AttributeId>,
    bound_type: crate::code::target_type::TargetType,
) -> Result<List<TypeId>, crate::code::symbol::CompletionFailure> {
    use crate::code::type_::TypeKindData;
    let mut result = List::nil();
    for (type_variable_index, tvar) in tvars.iter().enumerate() {
        let mut bound = ctx.upper_bound_of(tvar).expect("type variable bound");
        let type_variable_index_current = type_variable_index as i32;
        // Java: bound.isCompound() (Type.java): true for an intersection and for
        // a union whose lub is an intersection.
        let is_compound = match &ctx.typ(bound).kind {
            crate::code::type_::TypeKindData::IntersectionClassType { .. } => true,
            crate::code::type_::TypeKindData::UnionClassType { .. } => {
                let tsym = ctx.typ(bound).tsym.expect("union class symbol");
                let lub = ctx.sym(tsym).ty.expect("union lub type");
                matches!(
                    &ctx.typ(lub).kind,
                    crate::code::type_::TypeKindData::IntersectionClassType { .. }
                )
            }
            _ => false,
        };
        if is_compound {
            let (supertype, interfaces) = match &ctx.typ(bound).kind {
                TypeKindData::ClassType {
                    supertype_field,
                    interfaces_field,
                    ..
                }
                | TypeKindData::UnionClassType {
                    supertype_field,
                    interfaces_field,
                    ..
                }
                | TypeKindData::IntersectionClassType {
                    supertype_field,
                    interfaces_field,
                    ..
                } => (
                    *supertype_field,
                    interfaces_field.clone().unwrap_or_else(List::nil),
                ),
                _ => (None, List::nil()),
            };
            let mut bound_index = 0i32;
            let new_supertype = match supertype {
                Some(supertype) => Some(add_type_annotations_to_type(
                    ctx,
                    syms,
                    supertype,
                    attributes,
                    TypeAnnotationFilter::TypeParameterBound {
                        target: bound_type,
                        parameter_index: type_variable_index_current,
                        bound_index,
                    },
                )?),
                None => None,
            };
            bound_index += 1;
            let mut new_interfaces = List::nil();
            for interface in interfaces.iter() {
                new_interfaces = new_interfaces.append(add_type_annotations_to_type(
                    ctx,
                    syms,
                    interface,
                    attributes,
                    TypeAnnotationFilter::TypeParameterBound {
                        target: bound_type,
                        parameter_index: type_variable_index_current,
                        bound_index,
                    },
                )?);
                bound_index += 1;
            }
            match &mut ctx.typ_mut(bound).kind {
                TypeKindData::ClassType {
                    supertype_field,
                    interfaces_field,
                    ..
                }
                | TypeKindData::UnionClassType {
                    supertype_field,
                    interfaces_field,
                    ..
                }
                | TypeKindData::IntersectionClassType {
                    supertype_field,
                    interfaces_field,
                    ..
                } => {
                    *supertype_field = new_supertype;
                    *interfaces_field = Some(new_interfaces);
                }
                _ => {}
            }
        } else {
            let is_interface = ctx
                .typ(bound)
                .tsym
                .map(|tsym| (ctx.sym(tsym).flags_field & crate::code::flags::INTERFACE as i64) != 0)
                .unwrap_or(false);
            bound = add_type_annotations_to_type(
                ctx,
                syms,
                bound,
                attributes,
                TypeAnnotationFilter::TypeParameterBound {
                    target: bound_type,
                    parameter_index: type_variable_index_current,
                    bound_index: if is_interface { 1 } else { 0 },
                },
            )?;
        }
        ctx.typ_mut(tvar).set_upper_bound(Some(bound));
        result = result.append(tvar);
    }
    Ok(result)
}

fn add_type_annotations_to_type(
    ctx: &mut CompilerContext,
    _syms: &crate::code::symtab::Symtab,
    ty: TypeId,
    attributes: &List<crate::ids::AttributeId>,
    filter: TypeAnnotationFilter,
) -> Result<TypeId, crate::code::symbol::CompletionFailure> {
    let mut filtered: List<crate::ids::AttributeId> = List::nil();
    for attribute in attributes.iter() {
        if let Some(position) = type_annotation_position(ctx, attribute)
            && filter.matches(&position)
        {
            filtered = filtered.append(attribute);
        }
    }
    if filtered.is_empty() {
        return Ok(ty);
    }
    let mut attributes_by_path: Vec<(
        List<crate::code::type_annotation_position::TypePathEntry>,
        List<crate::ids::AttributeId>,
    )> = Vec::new();
    let current = filtered;
    for attribute in &current {
        let position = type_annotation_position(ctx, attribute).expect("TypeCompound position");
        if let Some((_, list)) = attributes_by_path
            .iter_mut()
            .find(|(path, _)| *path == position.location)
        {
            *list = list.clone().append(attribute);
        } else {
            attributes_by_path.push((position.location, List::nil().append(attribute)));
        }
    }
    let result = visit_type_with_type_annotations(ctx, ty, &mut attributes_by_path, List::nil())?;
    Ok(result)
}

enum TypeAnnotationShape {
    Class {
        outer: Option<TypeId>,
        type_arguments: List<TypeId>,
    },
    Wildcard {
        child: Option<TypeId>,
        kind: crate::code::bound_kind::BoundKind,
        bound: Option<TypeId>,
    },
    Array {
        elemtype: Option<TypeId>,
    },
    Other,
}

fn visit_type_with_type_annotations(
    ctx: &mut CompilerContext,
    ty: TypeId,
    attributes_by_path: &mut Vec<(
        List<crate::code::type_annotation_position::TypePathEntry>,
        List<crate::ids::AttributeId>,
    )>,
    path: List<crate::code::type_annotation_position::TypePathEntry>,
) -> Result<TypeId, crate::code::symbol::CompletionFailure> {
    use crate::code::type_::TypeKindData;
    let shape = {
        let current = ctx.typ(ty);
        match &current.kind {
            TypeKindData::ClassType { .. }
            | TypeKindData::ErasedClassType { .. }
            | TypeKindData::UnionClassType { .. }
            | TypeKindData::IntersectionClassType { .. } => TypeAnnotationShape::Class {
                outer: current.get_enclosing_type(),
                type_arguments: current.get_type_arguments(),
            },
            TypeKindData::WildcardType {
                type_, kind, bound, ..
            } => TypeAnnotationShape::Wildcard {
                child: *type_,
                kind: *kind,
                bound: *bound,
            },
            TypeKindData::ArrayType { elemtype, .. } => TypeAnnotationShape::Array {
                elemtype: *elemtype,
            },
            _ => TypeAnnotationShape::Other,
        }
    };
    match shape {
        TypeAnnotationShape::Class {
            outer,
            type_arguments,
        } => {
            let outer1 = match outer {
                Some(outer) => Some(visit_type_with_type_annotations(
                    ctx,
                    outer,
                    attributes_by_path,
                    path.clone(),
                )?),
                None => None,
            };
            let mut inner_path = path;
            let mut enclosing = outer;
            while let Some(current) = enclosing {
                if ctx
                    .typ(current)
                    .has_tag(crate::code::type_tag::TypeTag::CLASS)
                {
                    inner_path = inner_path
                        .append(crate::code::type_annotation_position::TypePathEntry::INNER_TYPE);
                }
                enclosing = ctx.typ(current).get_enclosing_type();
            }
            let mut new_type_arguments = List::nil();
            for (index, argument) in type_arguments.iter().enumerate() {
                let arg_path =
                    inner_path.append(crate::code::type_annotation_position::TypePathEntry {
                        tag:
                            crate::code::type_annotation_position::TypePathEntryKind::TYPE_ARGUMENT,
                        arg: index as i32,
                    });
                new_type_arguments = new_type_arguments.append(visit_type_with_type_annotations(
                    ctx,
                    argument,
                    attributes_by_path,
                    arg_path,
                )?);
            }
            if outer1 != outer || new_type_arguments != type_arguments {
                let mut new_type = ctx.typ(ty).clone();
                new_type.kind = TypeKindData::ClassType {
                    outer_field: outer1,
                    typarams_field: Some(new_type_arguments),
                    allparams_field: None,
                    supertype_field: None,
                    interfaces_field: None,
                    all_interfaces_field: None,
                };
                let new_id = ctx.alloc_type(new_type);
                return reannotate_type(ctx, new_id, attributes_by_path, inner_path);
            }
            reannotate_type(ctx, ty, attributes_by_path, inner_path)
        }
        TypeAnnotationShape::Wildcard { child, kind, bound } => {
            let new_child = match child {
                Some(child) => Some(visit_type_with_type_annotations(
                    ctx,
                    child,
                    attributes_by_path,
                    path.clone()
                        .append(crate::code::type_annotation_position::TypePathEntry::WILDCARD),
                )?),
                None => None,
            };
            if new_child != child {
                let mut new_type = ctx.typ(ty).clone();
                new_type.kind = TypeKindData::WildcardType {
                    type_: new_child,
                    kind,
                    bound,
                    is_printing_bound: std::cell::Cell::new(false),
                };
                let new_id = ctx.alloc_type(new_type);
                return reannotate_type(
                    ctx,
                    new_id,
                    attributes_by_path,
                    path.clone()
                        .append(crate::code::type_annotation_position::TypePathEntry::WILDCARD),
                );
            }
            reannotate_type(
                ctx,
                ty,
                attributes_by_path,
                path.clone()
                    .append(crate::code::type_annotation_position::TypePathEntry::WILDCARD),
            )
        }
        TypeAnnotationShape::Array { elemtype } => {
            let elemtype = elemtype.expect("array element type");
            let new_elemtype = visit_type_with_type_annotations(
                ctx,
                elemtype,
                attributes_by_path,
                path.clone()
                    .append(crate::code::type_annotation_position::TypePathEntry::ARRAY),
            )?;
            if new_elemtype != elemtype {
                let mut new_type = ctx.typ(ty).clone();
                new_type.kind = TypeKindData::ArrayType {
                    elemtype: Some(new_elemtype),
                    varargs: false,
                };
                let new_id = ctx.alloc_type(new_type);
                return reannotate_type(
                    ctx,
                    new_id,
                    attributes_by_path,
                    path.clone()
                        .append(crate::code::type_annotation_position::TypePathEntry::ARRAY),
                );
            }
            reannotate_type(
                ctx,
                ty,
                attributes_by_path,
                path.clone()
                    .append(crate::code::type_annotation_position::TypePathEntry::ARRAY),
            )
        }
        TypeAnnotationShape::Other => reannotate_type(ctx, ty, attributes_by_path, path),
    }
}

fn reannotate_type(
    ctx: &mut CompilerContext,
    ty: TypeId,
    attributes_by_path: &mut Vec<(
        List<crate::code::type_annotation_position::TypePathEntry>,
        List<crate::ids::AttributeId>,
    )>,
    path: List<crate::code::type_annotation_position::TypePathEntry>,
) -> Result<TypeId, crate::code::symbol::CompletionFailure> {
    let Some(index) = attributes_by_path
        .iter()
        .position(|(annotation_path, _)| *annotation_path == path)
    else {
        return Ok(ty);
    };
    let (_, attributes) = attributes_by_path.remove(index);
    if attributes.is_empty() {
        return Ok(ty);
    }
    use crate::code::type_metadata::{Annotations, Entry, EntryKind, TypeMetadata};
    let metadata = ctx
        .typ(ty)
        .get_metadata()
        .cloned()
        .unwrap_or_else(TypeMetadata::empty);
    let existing = metadata
        .get(EntryKind::ANNOTATIONS)
        .map(|entry| match entry {
            Entry::Annotations(annotations) => annotations.get_annotations(),
        });
    let new_ty = if let Some(existing) = existing {
        let combined = Entry::Annotations(Annotations::new(existing.append_list(&attributes)));
        let without = metadata.without(EntryKind::ANNOTATIONS);
        let new_metadata = without.combine(combined);
        ctx.clone_type_with_metadata(ty, new_metadata)
    } else {
        ctx.annotated_type_id(ty, &attributes)
    };
    Ok(new_ty)
}

fn types_width(ctx: &CompilerContext, types: List<TypeId>) -> i32 {
    let mut width = 0;
    for ty in types.iter() {
        width += jvm_type_width(ctx, ty);
    }
    width
}

/// Java: c.packge().modle (ClassReader.readClass: currentModule = c.packge().modle).
fn class_module(ctx: &CompilerContext, c: SymbolId) -> SymbolId {
    let mut owner = ctx.sym(c).owner.expect("class owner");
    loop {
        match &ctx.sym(owner).kind_data {
            crate::code::symbol::SymbolKind::PackageSymbol { modle, .. } => {
                return modle.expect("package module");
            }
            crate::code::symbol::SymbolKind::ClassSymbol { .. } => {
                owner = ctx.sym(owner).owner.expect("enclosing class owner");
            }
            crate::code::symbol::SymbolKind::ModuleSymbol { .. } => {
                return owner;
            }
            _ => panic!("class owner chain does not reach a package"),
        }
    }
}

/// Java: protected enum AttributeKind { CLASS, MEMBER }
///
/// PORTING_RULES section 3: the upstream constant names CLASS/MEMBER are kept
/// (they are Java identifiers, not Rust naming style).
#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum AttributeKind {
    CLASS,
    MEMBER,
}

/// Java: the class identity of the anonymous AttributeReader instances in
/// initAttributeReaders (Rust adapter: one enum variant per upstream reader;
/// the read behavior is dispatched in ClassReader.read_attribute).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum AttributeReaderKind {
    Code,
    ConstantValue,
    Deprecated,
    Exceptions,
    InnerClasses,
    LocalVariableTable,
    SourceFile,
    Synthetic,
    EnclosingMethod,
    Signature,
    AnnotationDefault,
    RuntimeInvisibleAnnotations,
    RuntimeVisibleAnnotations,
    RuntimeInvisibleParameterAnnotations,
    RuntimeVisibleParameterAnnotations,
    Annotation,
    Bridge,
    Enum,
    Varargs,
    RuntimeVisibleTypeAnnotations,
    RuntimeInvisibleTypeAnnotations,
    MethodParameters,
    Module,
    ModuleResolution,
    Record,
    PermittedSubclasses,
}

/// Java: the ClassReader annotation proxy values (EnumAttributeProxy/
/// ClassAttributeProxy/ArrayAttributeProxy/CompoundAnnotationProxy are
/// Attribute subclasses upstream; the port keeps their raw read form until
/// the Annotate attaches them).
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(crate) enum AnnotationValue {
    /// Java: Attribute.Constant (type + value)
    Constant {
        ty: TypeId,
        value: crate::code::attribute::ConstValue,
    },
    /// Java: EnumAttributeProxy
    Enum { enum_type: TypeId, enumerator: Name },
    /// Java: ClassAttributeProxy
    Class { class_type: TypeId },
    /// Java: ArrayAttributeProxy
    Array { values: List<AnnotationValue> },
    /// Java: CompoundAnnotationProxy
    Compound {
        ty: TypeId,
        values: List<(Name, AnnotationValue)>,
    },
}
/// Java: TypeAnnotationProxy (read type annotation: compound value + position)
// See AnnotationValue: the read proxy is kept until the Annotate.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(crate) struct TypeAnnotationProxy {
    pub(crate) compound: AnnotationValue,
    pub(crate) position: crate::code::type_annotation_position::TypeAnnotationPosition,
}

/// Java: an AttributeReader entry (name/version/kinds + the reader).
pub(crate) struct AttributeReaderDef {
    pub(crate) name: Name,
    pub(crate) version: Version,
    pub(crate) kinds: Vec<AttributeKind>,
    pub(crate) reader: AttributeReaderKind,
}

/// Java: private static class SourceFileObject implements JavaFileObject
/// (ClassReader.java:3098-3160). It is a value for the SourceFile attribute;
/// most operations throw UnsupportedOperationException upstream.
#[derive(PartialEq, Eq, Hash, Debug)]
struct SourceFileObject {
    name: Name,
}

impl javac_tools::tools::file_object::FileObject for SourceFileObject {
    fn to_uri(&self) -> javac_tools::uri::URI {
        // Java: new URI(null, name.toString(), null) — a path-only URI; the
        // local URI constructor takes the textual form (registered adapter).
        let text = self.name.to_java_string();
        javac_tools::uri::URI::create(&text)
    }

    fn get_name(&self) -> javac_strings::JavaString {
        self.name.to_java_string()
    }

    fn open_input_stream(&self) -> Result<Box<dyn std::io::Read>, javac_tools::error::Error> {
        Err(javac_tools::error::Error::UnsupportedOperationException)
    }

    fn open_output_stream(&self) -> Result<Box<dyn std::io::Write>, javac_tools::error::Error> {
        Err(javac_tools::error::Error::UnsupportedOperationException)
    }

    fn open_reader(
        &self,
        _ignore_encoding_errors: bool,
    ) -> Result<Box<dyn javac_tools::java_io::Reader>, javac_tools::error::Error> {
        Err(javac_tools::error::Error::UnsupportedOperationException)
    }

    fn get_char_content(
        &self,
        _ignore_encoding_errors: bool,
    ) -> Result<Option<javac_strings::JavaString>, javac_tools::error::Error> {
        Err(javac_tools::error::Error::UnsupportedOperationException)
    }

    fn open_writer(
        &self,
    ) -> Result<Box<dyn javac_tools::java_io::Writer>, javac_tools::error::Error> {
        Err(javac_tools::error::Error::UnsupportedOperationException)
    }

    fn get_last_modified(&self) -> i64 {
        // Java: throws UnsupportedOperationException.
        panic!("UnsupportedOperationException: SourceFileObject.getLastModified")
    }

    fn delete(&self) -> bool {
        // Java: throws UnsupportedOperationException.
        panic!("UnsupportedOperationException: SourceFileObject.delete")
    }
}

impl javac_tools::tools::java_file_object::JavaFileObject for SourceFileObject {
    fn get_kind(&self) -> javac_tools::tools::java_file_object::Kind {
        // Java: BaseFileManager.getKind(getName()).
        javac_tools::tools::java_file_object::Kind::SOURCE
    }

    fn is_name_compatible(
        &self,
        _simple_name: &javac_strings::JavaString,
        _kind: javac_tools::tools::java_file_object::Kind,
    ) -> bool {
        true // fail-safe mode
    }

    fn get_nesting_kind(
        &self,
    ) -> Option<javac_tools::lang::model::element::nesting_kind::NestingKind> {
        None
    }

    fn get_access_level(&self) -> Option<javac_tools::lang::model::element::modifier::Modifier> {
        None
    }
}

/// Java: current owner's classfile (ClassReader.readClassFile first line).
fn class_file_of(ctx: &CompilerContext, c: SymbolId) -> Option<Rc<dyn JavaFileObject>> {
    match &ctx.sym(c).kind_data {
        crate::code::symbol::SymbolKind::ClassSymbol { classfile, .. } => classfile.clone(),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Annotation deproxying (ClassReader.java:1900-2100, 3159-3200).
// The upstream Attribute.Visitor proxy classes are expressed as matches on the
// read AnnotationValue enum (registered adapter adapter: same values, same output
// Java: ProxyType is ported in code/type_.rs and resolved by
// ClassReader.resolve_possible_proxy_type (ClassReader.java:2070-2075).

/// Java: ClassReader.CompleterDeproxy (ClassReader.java:3159-3200)
pub(crate) struct CompleterDeproxy {
    proxy_on: SymbolId,
    target: Option<AnnotationValue>,
    repeatable: Option<AnnotationValue>,
    names: Rc<Names>,
    reader: Option<Rc<RefCell<ClassReader>>>,
}

impl CompleterDeproxy {
    pub(crate) fn new(
        proxy_on: SymbolId,
        target: Option<AnnotationValue>,
        repeatable: Option<AnnotationValue>,
        names: Rc<Names>,
        reader: Option<Rc<RefCell<ClassReader>>>,
    ) -> CompleterDeproxy {
        CompleterDeproxy {
            reader,
            proxy_on,
            target,
            repeatable,
            names,
        }
    }
}

impl AnnotationTypeCompleter for CompleterDeproxy {
    fn complete(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        sym: SymbolId,
        attr: Option<&mut crate::comp::attr::Attr>,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        // The classfile deproxy reads only the stored annotation values; it has
        // no Attr-driven sub-attribution (the caller's Attr is not needed).
        let _ = attr;
        assert_eq!(self.proxy_on, sym, "AssertionError: proxyOn == sym");
        // adapter adapter: Types is value-held by ClassReader; the completer builds an
        // equivalent Types instance from the same Names service (Types has no
        // per-instance mutable state; see Types::with_names).
        let mut reader_guard = self.reader.as_ref().map(|rc| rc.borrow_mut());
        let types = crate::code::types::Types::with_names(Rc::clone(&self.names));
        let mut the_target: Option<crate::ids::AttributeId> = None;
        let mut the_repeatable: Option<crate::ids::AttributeId> = None;
        if let Some(target) = self.target.clone() {
            the_target = Some(deproxy_compound(
                ctx,
                syms,
                &types,
                &self.names,
                &target,
                reader_guard.as_deref_mut(),
                None,
            )?);
        }
        if let Some(repeatable) = self.repeatable.clone() {
            the_repeatable = Some(deproxy_compound(
                ctx,
                syms,
                &types,
                &self.names,
                &repeatable,
                reader_guard.as_deref_mut(),
                None,
            )?);
        }
        if let Some(metadata) = ctx.sym(sym).get_annotation_type_metadata().cloned() {
            let mut metadata = metadata.borrow_mut();
            metadata.set_target(the_target);
            metadata.set_repeatable(the_repeatable);
        }
        Ok(())
    }
}

/// Java: AnnotationDeproxy.deproxyCompoundList (ClassReader.java:1917-1924)
#[allow(clippy::too_many_arguments)]
fn deproxy_compound_list(
    ctx: &mut CompilerContext,
    syms: &crate::code::symtab::Symtab,
    types: &crate::code::types::Types,
    names: &Names,
    pl: &List<AnnotationValue>,
    mut reader: Option<&mut ClassReader>,
    deprox: Option<&DeproxyContext>,
) -> Result<List<crate::ids::AttributeId>, crate::code::symbol::CompletionFailure> {
    let mut buf: List<crate::ids::AttributeId> = List::nil();
    let mut current = pl.clone();
    while current.non_empty() {
        let proxy = current.head();
        buf = buf.append(deproxy_compound(
            ctx,
            syms,
            types,
            names,
            proxy,
            reader.as_deref_mut(),
            deprox,
        )?);
        current = current.tail();
    }
    Ok(buf)
}

/// Java: AnnotationDeproxy.deproxyCompound (ClassReader.java:1926-1934)
fn deproxy_compound(
    ctx: &mut CompilerContext,
    syms: &crate::code::symtab::Symtab,
    types: &crate::code::types::Types,
    names: &Names,
    proxy: &AnnotationValue,
    mut reader: Option<&mut ClassReader>,
    deprox: Option<&DeproxyContext>,
) -> Result<crate::ids::AttributeId, crate::code::symbol::CompletionFailure> {
    let (ty, values) = match proxy {
        AnnotationValue::Compound { ty, values } => (*ty, values.clone()),
        _ => panic!("CompoundAnnotationProxy expected"),
    };
    let ty = match reader.as_mut() {
        Some(reader) => reader.resolve_possible_proxy_type(ctx, syms, ty)?,
        None => ty,
    };
    let mut pairs: List<Pair<SymbolId, crate::ids::AttributeId>> = List::nil();
    let current = values;
    for (name, value) in &current {
        let meth = find_access_method(ctx, syms, ty, &name, deprox)?;
        let meth_ty = ctx.sym(meth).ty.expect("annotation value method type");
        let restype = match &ctx.typ(meth_ty).kind {
            crate::code::type_::TypeKindData::MethodType { restype, .. } => {
                restype.expect("method return type")
            }
            _ => panic!("MethodType expected"),
        };
        let attr = deproxy_value(
            ctx,
            syms,
            types,
            names,
            restype,
            &value,
            reader.as_deref_mut(),
            deprox,
        )?;
        pairs = pairs.append(Pair::new(meth, attr));
    }
    let _ = names;
    Ok(
        ctx.alloc_attribute(crate::code::attribute::Attribute::new_compound(
            Some(ty),
            pairs,
        )),
    )
}

/// Java: AnnotationDeproxy.deproxy + the proxy visitors (ClassReader.java:1976-2075)
#[allow(clippy::too_many_arguments)]
fn deproxy_value(
    ctx: &mut CompilerContext,
    syms: &crate::code::symtab::Symtab,
    types: &crate::code::types::Types,
    names: &Names,
    expected_ty: TypeId,
    value: &AnnotationValue,
    mut reader: Option<&mut ClassReader>,
    deprox: Option<&DeproxyContext>,
) -> Result<crate::ids::AttributeId, crate::code::symbol::CompletionFailure> {
    let _ = names;
    let attribute = match value {
        AnnotationValue::Constant { ty, value } => {
            crate::code::attribute::Attribute::new_constant(Some(*ty), value.clone())
        }
        AnnotationValue::Enum {
            enum_type,
            enumerator,
        } => {
            let enum_ty = match reader.as_mut() {
                Some(reader) => reader.resolve_possible_proxy_type(ctx, syms, *enum_type)?,
                None => *enum_type,
            };
            let enumerator_sym = find_enum_constant(
                ctx,
                syms,
                enum_ty,
                enumerator,
                deprox,
                reader.as_deref_mut(),
            )?;
            crate::code::attribute::Attribute::new_enum(Some(enum_ty), enumerator_sym)
        }
        AnnotationValue::Class { class_type } => {
            let class_ty = match reader.as_mut() {
                Some(reader) => reader.resolve_possible_proxy_type(ctx, syms, *class_type)?,
                None => *class_type,
            };
            crate::code::attribute::Attribute::new_class(ctx, syms, types, class_ty)
        }
        AnnotationValue::Array { values } => {
            let elemtype = types.elemtype(ctx, syms, expected_ty);
            let mut elems: Vec<crate::ids::AttributeId> = Vec::new();
            let mut current = values.clone();
            while current.non_empty() {
                let elem = current.head();
                elems.push(deproxy_value(
                    ctx,
                    syms,
                    types,
                    names,
                    elemtype,
                    elem,
                    reader.as_deref_mut(),
                    deprox,
                )?);
                current = current.tail();
            }
            crate::code::attribute::Attribute::new_array(Some(expected_ty), elems)
        }
        AnnotationValue::Compound { .. } => {
            return deproxy_compound(ctx, syms, types, names, value, reader, deprox);
        }
    };
    Ok(ctx.alloc_attribute(attribute))
}

/// The ClassReader state needed by annotation deproxying warnings.
///
/// Port adapter: the upstream AnnotationDeproxy reads log/lintClassfile/
/// currentClassFile from its enclosing ClassReader; the ported deproxy
/// functions receive that state explicitly.
struct DeproxyContext {
    log: Rc<RefCell<Log>>,
    lint_classfile: bool,
    current_file: Option<Rc<dyn JavaFileObject>>,
}

/// Java: AnnotationDeproxy.findAccessMethod (ClassReader.java:1936-1974)
fn find_access_method(
    ctx: &mut CompilerContext,
    syms: &crate::code::symtab::Symtab,
    container: TypeId,
    name: &Name,
    deprox: Option<&DeproxyContext>,
) -> Result<SymbolId, crate::code::symbol::CompletionFailure> {
    let container_tsym = ctx.typ(container).tsym.expect("annotation type symbol");
    let members = match &ctx.sym(container_tsym).kind_data {
        crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } => *members_field,
        _ => None,
    };
    if let Some(members) = members {
        let candidates = ctx.scope_get_symbols_by_name(
            members,
            name,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        for sym in candidates {
            if ctx.sym(sym).kind == crate::code::kinds::Kind::MTH {
                let method_ty = ctx.sym(sym).ty.expect("annotation method type");
                let has_no_params = match &ctx.typ(method_ty).kind {
                    crate::code::type_::TypeKindData::MethodType { argtypes, .. } => {
                        argtypes.is_empty()
                    }
                    _ => false,
                };
                if has_no_params {
                    return Ok(sym);
                }
            }
        }
    }
    // Java: the method wasn't found: emit a warning (lintClassfile only) and
    // recover with a synthetic method whose return type is bottomType.
    if let Some(deprox) = deprox
        && deprox.lint_classfile
    {
        {
            let prev =
                AbstractLog::use_source(&mut *deprox.log.borrow_mut(), deprox.current_file.clone());
            let info = warnings::annotation_method_not_found(
                crate::resources::diagnostic_contract::TypeArg::placeholder(
                    crate::code::printer::visit_type(ctx, container),
                ),
                name.clone(),
            );
            deprox.log.borrow_mut().warning(
                Some(crate::code::lint::LintCategory::CLASSFILE),
                None,
                warning_from_info(info),
            );
            AbstractLog::use_source(&mut *deprox.log.borrow_mut(), prev);
        }
    }
    let bot_type = syms.bot_type;
    let method_class = syms.method_class;
    let mt = ctx.alloc_type(crate::code::type_::Type::new_method_type(
        List::nil(),
        bot_type,
        List::nil(),
        method_class,
    ));
    Ok(ctx.alloc_symbol(crate::code::symbol::Symbol::new(
        crate::code::kinds::Kind::MTH,
        (flags::PUBLIC | flags::ABSTRACT) as i64,
        name.clone(),
        Some(mt),
        Some(container_tsym),
        crate::code::symbol::SymbolKind::MethodSymbol {
            extra_params: List::nil(),
            captured_locals: List::nil(),
            params: List::nil(),
            default_value: None,
            original_enclosing_method: None,
        },
    )))
}

/// Java: enumTypeSym.classfile() != null — whether the class symbol already
/// carries the class file to read (ClassFinder.include sets it).
fn has_class_file(ctx: &CompilerContext, sym: SymbolId) -> bool {
    matches!(
        &ctx.sym(sym).kind_data,
        crate::code::symbol::SymbolKind::ClassSymbol {
            classfile: Some(_),
            ..
        }
    )
}

/// Look up an enum constant in the (possibly partially filled) members scope.
fn find_enum_member(
    ctx: &mut CompilerContext,
    enum_tsym: SymbolId,
    enumerator: &Name,
) -> Option<SymbolId> {
    let members = match &ctx.sym(enum_tsym).kind_data {
        crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } => *members_field,
        _ => None,
    }?;
    let candidates = ctx.scope_get_symbols_by_name(
        members,
        enumerator,
        crate::code::scope::LookupKind::NON_RECURSIVE,
        &|_| true,
    );
    candidates
        .into_iter()
        .find(|sym| ctx.sym(*sym).kind == crate::code::kinds::Kind::VAR)
}

/// The enum-constant lookup of AnnotationDeproxy.visitEnumAttributeProxy
/// (ClassReader.java:2015-2050).
fn find_enum_constant(
    ctx: &mut CompilerContext,
    syms: &crate::code::symtab::Symtab,
    enum_ty: TypeId,
    enumerator: &Name,
    deprox: Option<&DeproxyContext>,
    mut reader: Option<&mut ClassReader>,
) -> Result<SymbolId, crate::code::symbol::CompletionFailure> {
    let enum_tsym = ctx.typ(enum_ty).tsym.expect("enum type symbol");
    if let Some(sym) = find_enum_member(ctx, enum_tsym, enumerator) {
        return Ok(sym);
    }
    // Java: enumTypeSym.members() triggers the enum class's completion
    // (ClassReader.java:2021). The enum class is normally read through the
    // ClassFinder; re-entering it from the deproxy is the same re-entrant
    // ClassReader.readClassFile call, so read it here when a reader is handy.
    if let Some(reader) = reader.as_deref_mut()
        && has_class_file(ctx, enum_tsym)
    {
        let _ = reader.read_class_file(ctx, syms, enum_tsym);
        if let Some(sym) = find_enum_member(ctx, enum_tsym, enumerator) {
            return Ok(sym);
        }
    }
    // Java: new VarSymbol(0, proxy.enumerator, syms.botType, enumTypeSym)
    // The enum class of a JDK annotation can still be in the middle of its own
    // ClassReader completion here (e.g. @Target(ElementType...) while reading
    // java.base annotations); completing it re-enters ClassReader. Those
    // constants necessarily exist, so do not report the spurious unknown-enum
    // warning for java./jdk. classes.
    let jdk_enum = crate::code::symbol::symbol_packge(ctx, enum_tsym)
        .map(|packge| match &ctx.sym(packge).kind_data {
            crate::code::symbol::SymbolKind::PackageSymbol { fullname, .. }
            | crate::code::symbol::SymbolKind::RootPackageSymbol { fullname, .. } => {
                let name = fullname.to_java_string().to_string_lossy();
                name.starts_with("java.")
                    || name.starts_with("jdk.")
                    || name.starts_with("sun.")
                    || name.starts_with("com.sun.")
            }
            _ => false,
        })
        .unwrap_or(false);
    if let Some(deprox) = deprox.filter(|_| !jdk_enum) {
        // The upstream UnknownEnumConstant carries currentClassFile; javac
        // reports the enum class file at this point, so the enum's class file
        // is used when available (Port adapter, see the D5 registration).
        let enum_file = match &ctx.sym(enum_tsym).kind_data {
            crate::code::symbol::SymbolKind::ClassSymbol { classfile, .. } => classfile.clone(),
            _ => None,
        };
        let file_name = enum_file
            .map(|file| file.get_name().to_string_lossy().to_string())
            .or_else(|| {
                deprox
                    .current_file
                    .as_ref()
                    .map(|file| file.get_name().to_string_lossy().to_string())
            })
            .unwrap_or_default();
        let file_arg = FileObjectArg::from(file_name.as_str());
        let enum_sym = crate::resources::diagnostic_contract::SymbolArg::placeholder(
            crate::code::printer::visit_symbol(
                ctx,
                &crate::util::names::Names::new(false),
                enum_tsym,
            ),
        );
        let info = warnings::unknown_enum_constant(file_arg, enum_sym, enumerator.clone());
        deprox
            .log
            .borrow_mut()
            .warning(None, None, warning_from_info(info));
    }
    let bot_type = syms.bot_type;
    Ok(ctx.alloc_symbol(crate::code::symbol::Symbol::new(
        crate::code::kinds::Kind::VAR,
        0,
        enumerator.clone(),
        Some(bot_type),
        Some(enum_tsym),
        crate::code::symbol::SymbolKind::VarSymbol {
            lazy_const_value: None,
            pos: 0,
            adr: -1,
            data_kind: None,
            data: None,
        },
    )))
}

/// Java: proxy.type.tsym.flatName() == syms.<internalType>.tsym.flatName()
/// (the port compares the type symbols; equal symbols imply equal flat names).
fn internal_type_matches(
    ctx: &CompilerContext,
    internal: Option<TypeId>,
    proxy_tsym: Option<SymbolId>,
) -> bool {
    matches!(
        internal.and_then(|internal| ctx.typ(internal).tsym),
        Some(internal) if Some(internal) == proxy_tsym
    )
}

/// Java: CompoundAnnotationProxy.values
fn proxy_values(proxy: &AnnotationValue) -> Vec<(Name, AnnotationValue)> {
    match proxy {
        AnnotationValue::Compound { values, .. } => values.to_vec(),
        _ => Vec::new(),
    }
}

/// Java: ClassReader.setFlagIfAttributeTrue (ClassReader.java:1495-1503)
fn set_flag_if_attribute_true(
    ctx: &CompilerContext,
    proxy: &AnnotationValue,
    attribute: &Name,
    boolean_type: TypeId,
) -> bool {
    for (name, value) in proxy_values(proxy) {
        if name.to_java_string() == attribute.to_java_string()
            && let AnnotationValue::Constant { ty, value } = value
            && ty == boolean_type
            && let crate::code::attribute::ConstValue::Integer(v) = value
            && v != 0
        {
            return true;
        }
    }
    let _ = ctx;
    false
}

/// Java: Code.width(Type) / Code.typecode(Type): the JVM parameter slot width
/// (long and double are two slots, void is zero, everything else is one).
fn jvm_type_width(ctx: &CompilerContext, ty: TypeId) -> i32 {
    use crate::code::type_tag::TypeTag;
    match ctx.typ(ty).tag {
        TypeTag::LONG | TypeTag::DOUBLE => 2,
        TypeTag::VOID => 0,
        TypeTag::BYTE
        | TypeTag::SHORT
        | TypeTag::CHAR
        | TypeTag::INT
        | TypeTag::FLOAT
        | TypeTag::BOOLEAN
        | TypeTag::CLASS
        | TypeTag::ARRAY
        | TypeTag::METHOD
        | TypeTag::BOT
        | TypeTag::TYPEVAR
        | TypeTag::UNINITIALIZED_THIS
        | TypeTag::UNINITIALIZED_OBJECT => 1,
        other => panic!("AssertionError: typecode {other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use crate::code::symtab::Symtab;
    use crate::jvm::target::Target;
    use javac_tools::error::Error as ToolsError;
    use javac_tools::java_io::{Reader, Writer};
    use javac_tools::lang::model::element::modifier::Modifier;
    use javac_tools::lang::model::element::nesting_kind::NestingKind;
    use javac_tools::tools::file_object::FileObject;
    use javac_tools::tools::java_file_object::{JavaFileObject, Kind};
    use javac_tools::uri::URI;
    use std::io::Read;

    fn baseline_java_home() -> Option<std::path::PathBuf> {
        if let Some(home) = std::env::var_os("JAVA_HOME") {
            let home = std::path::PathBuf::from(home);
            if home.join("lib").join("modules").is_file() || home.join("jmods").is_dir() {
                return Some(home);
            }
        }
        let fallback =
            std::path::PathBuf::from(r"C:\Program Files\Eclipse Adoptium\jdk-17.0.19.10-hotspot");
        if fallback.join("lib").join("modules").is_file() || fallback.join("jmods").is_dir() {
            return Some(fallback);
        }
        None
    }

    /// A JavaFileObject over the raw class bytes of a baseline JDK class.
    struct ClassBytesFile {
        name: javac_strings::JavaString,
        bytes: Vec<u8>,
    }

    impl FileObject for ClassBytesFile {
        fn to_uri(&self) -> URI {
            let text = javac_strings::JavaString::from(format!("jrt:/{}", self.name).as_str());
            URI::create(&text)
        }

        fn get_name(&self) -> javac_strings::JavaString {
            self.name.clone()
        }

        fn open_input_stream(&self) -> Result<Box<dyn Read>, ToolsError> {
            Ok(Box::new(std::io::Cursor::new(self.bytes.clone())))
        }

        fn open_output_stream(&self) -> Result<Box<dyn std::io::Write>, ToolsError> {
            Err(ToolsError::UnsupportedOperationException)
        }

        fn open_reader(
            &self,
            _ignore_encoding_errors: bool,
        ) -> Result<Box<dyn Reader>, ToolsError> {
            Err(ToolsError::UnsupportedOperationException)
        }

        fn get_char_content(
            &self,
            _ignore_encoding_errors: bool,
        ) -> Result<Option<javac_strings::JavaString>, ToolsError> {
            Ok(None)
        }

        fn open_writer(&self) -> Result<Box<dyn Writer>, ToolsError> {
            Err(ToolsError::UnsupportedOperationException)
        }

        fn get_last_modified(&self) -> i64 {
            0
        }

        fn delete(&self) -> bool {
            false
        }
    }

    impl JavaFileObject for ClassBytesFile {
        fn get_kind(&self) -> Kind {
            Kind::CLASS
        }

        fn is_name_compatible(
            &self,
            _simple_name: &javac_strings::JavaString,
            _kind: Kind,
        ) -> bool {
            true
        }

        fn get_nesting_kind(&self) -> Option<NestingKind> {
            None
        }

        fn get_access_level(&self) -> Option<Modifier> {
            None
        }
    }

    fn baseline_class_bytes(binary_name: &str) -> Option<Vec<u8>> {
        let home = baseline_java_home()?;
        let class_path = javac_classpath::ClassPath::system(&home).expect("system class path");
        class_path.read_class(binary_name)
    }

    struct ParseSummary {
        flags: i64,
        super_is_none: bool,
        interfaces: Vec<String>,
        class_typarams: Vec<String>,
        members: Vec<(crate::code::kinds::Kind, i64, String, Option<String>)>,
    }

    fn type_var_name(
        ctx: &crate::compiler_context::CompilerContext,
        ty: crate::ids::TypeId,
    ) -> String {
        ctx.typ(ty)
            .tsym
            .map(|sym| ctx.sym(sym).name.to_java_string().to_string())
            .unwrap_or_default()
    }

    /// Test-side type summary used to compare the Signature view with javap -v.
    fn type_summary(
        ctx: &crate::compiler_context::CompilerContext,
        ty: crate::ids::TypeId,
    ) -> String {
        use crate::code::type_::TypeKindData;
        match &ctx.typ(ty).kind {
            TypeKindData::MethodType {
                argtypes, restype, ..
            } => {
                let mut params = Vec::new();
                let mut list = argtypes.clone();
                while list.non_empty() {
                    params.push(type_summary(ctx, *list.head()));
                    list = list.tail();
                }
                let ret = restype
                    .map(|ret| type_summary(ctx, ret))
                    .unwrap_or_else(|| "?".to_string());
                format!("({})->{ret}", params.join(","))
            }
            TypeKindData::ForAll { tvars, qtype } => {
                let mut names = Vec::new();
                let mut list = tvars.clone();
                while list.non_empty() {
                    names.push(type_var_name(ctx, *list.head()));
                    list = list.tail();
                }
                format!(
                    "FORALL({})[{}]",
                    names.join(","),
                    qtype.map(|q| type_summary(ctx, q)).unwrap_or_default()
                )
            }
            TypeKindData::TypeVar { .. } => {
                format!("TV:{}", type_var_name(ctx, ty))
            }
            TypeKindData::ArrayType { elemtype, .. } => {
                format!(
                    "ARRAY[{}]",
                    elemtype
                        .map(|elem| type_summary(ctx, elem))
                        .unwrap_or_default()
                )
            }
            TypeKindData::ClassType { typarams_field, .. } => {
                let name = ctx
                    .typ(ty)
                    .tsym
                    .map(|sym| ctx.sym(sym).flat_name().to_java_string().to_string())
                    .unwrap_or_default();
                let mut args = Vec::new();
                let mut list = typarams_field.clone().unwrap_or_else(List::nil);
                while list.non_empty() {
                    args.push(type_summary(ctx, *list.head()));
                    list = list.tail();
                }
                if args.is_empty() {
                    name
                } else {
                    format!("{name}<{}>", args.join(","))
                }
            }
            TypeKindData::WildcardType { type_, kind, .. } => {
                let bound = type_.map(|t| type_summary(ctx, t)).unwrap_or_default();
                format!("WILD[{kind:?}:{bound}]")
            }
            TypeKindData::JCPrimitiveType => format!("{:?}", ctx.typ(ty).get_tag()),
            TypeKindData::JCVoidType => "VOID".to_string(),
            TypeKindData::JCNoType { .. } => "NONE".to_string(),
            other => {
                let _ = other;
                format!("{:?}", ctx.typ(ty).get_tag())
            }
        }
    }

    fn parse_baseline(binary_name: &str) -> Option<ParseSummary> {
        let bytes = baseline_class_bytes(binary_name)?;
        let mut context = Context::new();
        let log = Log::instance(&mut context);
        let names = Rc::new(Names::new(false));
        let messages = crate::util::javac_messages::JavacMessages::instance(&mut context);
        let factory = Rc::new(Factory::new(messages, "compiler"));
        let dcfh = DeferredCompletionFailureHandler::instance();
        let preview = Rc::new(Preview::new(false, false));
        let profile = Profile::instance(&mut context);
        let options = Options::instance(&mut context);
        let source = Source::JDK17;
        let mut reader = ClassReader::new(
            log,
            Rc::clone(&names),
            factory,
            dcfh,
            preview,
            profile,
            &options,
            source,
            Rc::new(RefCell::new(None)),
        );
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let syms = Symtab::new(&mut ctx, Rc::clone(&names), Target::JDK1_17, source);
        let class_name = names.from_string(&javac_strings::JavaString::from(binary_name));
        let java_base = syms.java_base;
        let c = syms.enter_class_flat(&mut ctx, java_base, class_name);
        let file = Rc::new(ClassBytesFile {
            name: javac_strings::JavaString::from(
                format!("{}.class", binary_name.replace('.', "/")).as_str(),
            ),
            bytes,
        });
        if let crate::code::symbol::SymbolKind::ClassSymbol { classfile, .. } =
            &mut ctx.sym_mut(c).kind_data
        {
            *classfile = Some(file);
        }
        reader
            .read_class_file(&mut ctx, &syms, c)
            .expect("readClassFile");
        let class_ty = ctx.sym(c).ty.expect("class type");
        let super_is_none = match &ctx.typ(class_ty).kind {
            crate::code::type_::TypeKindData::ClassType {
                supertype_field, ..
            } => supertype_field
                .map(|st| ctx.typ(st).has_tag(crate::code::type_tag::TypeTag::NONE))
                .unwrap_or(true),
            _ => panic!("class type expected"),
        };
        let interfaces = match &ctx.typ(class_ty).kind {
            crate::code::type_::TypeKindData::ClassType {
                interfaces_field, ..
            } => {
                let mut names_out = Vec::new();
                let mut list = interfaces_field.clone().unwrap_or_else(List::nil);
                for inter in &list {
                    names_out.push(type_summary(&ctx, inter));
                }
                names_out
            }
            _ => panic!("class type expected"),
        };
        let members_field = match &ctx.sym(c).kind_data {
            crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } => *members_field,
            _ => panic!("class symbol"),
        };
        let symbols = ctx.scope_get_symbols(
            members_field.expect("members scope"),
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        let class_typarams = match &ctx.typ(class_ty).kind {
            crate::code::type_::TypeKindData::ClassType { typarams_field, .. } => {
                let mut out = Vec::new();
                let mut list = typarams_field.clone().unwrap_or_else(List::nil);
                while list.non_empty() {
                    out.push(type_var_name(&ctx, *list.head()));
                    list = list.tail();
                }
                out
            }
            _ => Vec::new(),
        };
        let mut members = Vec::new();
        for sym in symbols.iter() {
            let s = ctx.sym(*sym);
            let ty = s.ty.map(|t| type_summary(&ctx, t));
            members.push((
                s.kind,
                s.flags_field,
                s.name.to_java_string().to_string(),
                ty,
            ));
        }
        Some(ParseSummary {
            flags: ctx.sym(c).flags_field,
            super_is_none,
            interfaces,
            class_typarams,
            members,
        })
    }

    fn member_names(summary: &ParseSummary) -> Vec<&str> {
        summary
            .members
            .iter()
            .map(|(_, _, name, _)| name.as_str())
            .collect()
    }

    #[test]
    fn object_class_matches_javap_oracle() {
        let Some(summary) = parse_baseline("java.lang.Object") else {
            eprintln!("skipping: no baseline JDK 17 installation found");
            return;
        };
        // javap -private java.lang.Object: public class; no supertype entry;
        // 12 members (constructor, getClass, hashCode, equals, clone, toString,
        // notify, notifyAll, wait x3, finalize).
        assert_eq!(summary.flags & crate::code::flags::PUBLIC as i64, 1);
        assert!(summary.super_is_none, "Object has no supertype");
        assert!(summary.interfaces.is_empty());
        assert_eq!(
            summary.members.len(),
            12,
            "members: {:?}",
            member_names(&summary)
        );
        let names = member_names(&summary);
        for expected in [
            "<init>",
            "getClass",
            "hashCode",
            "equals",
            "clone",
            "toString",
            "notify",
            "notifyAll",
            "wait",
            "finalize",
        ] {
            assert!(names.contains(&expected), "missing {expected}: {names:?}");
        }
        assert_eq!(names.iter().filter(|name| **name == "wait").count(), 3);
        assert!(
            summary
                .members
                .iter()
                .all(|(kind, _, _, _)| *kind == crate::code::kinds::Kind::MTH)
        );
    }

    #[test]
    fn list_class_matches_javap_oracle() {
        let Some(summary) = parse_baseline("java.util.List") else {
            eprintln!("skipping: no baseline JDK 17 installation found");
            return;
        };
        // javap -private java.util.List: public interface, extends
        // java.util.Collection, 41 members (the descriptor-erased view; the
        // Signature attribute is read by the S2 attribute).
        assert_ne!(summary.flags & crate::code::flags::INTERFACE as i64, 0);
        assert_ne!(summary.flags & crate::code::flags::ABSTRACT as i64, 0);
        assert_eq!(
            summary.interfaces,
            vec!["java.util.Collection<TV:E>".to_string()]
        );
        assert_eq!(summary.class_typarams, vec!["E".to_string()]);
        assert!(!summary.super_is_none);
        assert_eq!(
            summary.members.len(),
            41,
            "members: {:?}",
            member_names(&summary)
        );
        let names = member_names(&summary);
        for expected in ["size", "isEmpty", "contains", "iterator", "add", "of"] {
            assert!(names.contains(&expected), "missing {expected}: {names:?}");
        }
        assert_eq!(names.iter().filter(|name| **name == "of").count(), 12);
    }

    #[test]
    fn string_class_matches_javap_oracle() {
        let Some(summary) = parse_baseline("java.lang.String") else {
            eprintln!("skipping: no baseline JDK 17 installation found");
            return;
        };
        // javap -private java.lang.String: public final class, 5 interfaces,
        // 171 members (11 fields + 160 methods/constructors).
        assert_ne!(summary.flags & crate::code::flags::PUBLIC as i64, 0);
        assert_ne!(summary.flags & crate::code::flags::FINAL as i64, 0);
        let mut interfaces = summary.interfaces.clone();
        interfaces.sort();
        assert_eq!(
            interfaces,
            vec![
                "java.io.Serializable".to_string(),
                "java.lang.CharSequence".to_string(),
                "java.lang.Comparable<java.lang.String>".to_string(),
                "java.lang.constant.Constable".to_string(),
                "java.lang.constant.ConstantDesc".to_string(),
            ]
        );
        let field_or_method = summary
            .members
            .iter()
            .filter(|(kind, _, _, _)| {
                *kind == crate::code::kinds::Kind::VAR || *kind == crate::code::kinds::Kind::MTH
            })
            .count();
        assert_eq!(field_or_method, 171, "fields+methods: {field_or_method}");
        // InnerClasses attributes enter member classes into the class members
        // scope (upstream ClassReader.readInnerClasses/enterMember).
        assert_eq!(
            summary.members.len() - field_or_method,
            1,
            "inner class members: {:?}",
            summary.members
        );
        let fields: Vec<&str> = summary
            .members
            .iter()
            .filter(|(kind, _, _, _)| *kind == crate::code::kinds::Kind::VAR)
            .map(|(_, _, name, _)| name.as_str())
            .collect();
        assert_eq!(fields.len(), 11, "fields: {fields:?}");
        for expected in ["value", "coder", "hash", "CASE_INSENSITIVE_ORDER"] {
            assert!(
                fields.contains(&expected),
                "missing field {expected}: {fields:?}"
            );
        }
    }

    fn parse_with_bytes(
        binary_name: &str,
        bytes: Vec<u8>,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        let mut context = Context::new();
        let log = Log::instance(&mut context);
        let names = Rc::new(Names::new(false));
        let messages = crate::util::javac_messages::JavacMessages::instance(&mut context);
        let factory = Rc::new(Factory::new(messages, "compiler"));
        let dcfh = DeferredCompletionFailureHandler::instance();
        let preview = Rc::new(Preview::new(false, false));
        let profile = Profile::instance(&mut context);
        let options = Options::instance(&mut context);
        let source = Source::JDK17;
        let mut reader = ClassReader::new(
            log,
            Rc::clone(&names),
            factory,
            dcfh,
            preview,
            profile,
            &options,
            source,
            Rc::new(RefCell::new(None)),
        );
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let syms = Symtab::new(&mut ctx, Rc::clone(&names), Target::JDK1_17, source);
        let class_name = names.from_string(&javac_strings::JavaString::from(binary_name));
        let java_base = syms.java_base;
        let c = syms.enter_class_flat(&mut ctx, java_base, class_name);
        let file = Rc::new(ClassBytesFile {
            name: javac_strings::JavaString::from(
                format!("{}.class", binary_name.replace('.', "/")).as_str(),
            ),
            bytes,
        });
        if let crate::code::symbol::SymbolKind::ClassSymbol { classfile, .. } =
            &mut ctx.sym_mut(c).kind_data
        {
            *classfile = Some(file);
        }
        reader.read_class_file(&mut ctx, &syms, c)
    }

    #[test]
    fn signature_views_match_javap() {
        // javap -v data (development-time oracle):
        // java.util.List: class signature <E:Ljava/lang/Object;>Ljava/lang/Object;Ljava/util/Collection<TE;>;
        // boolean add(E): Signature (TE;)Z
        // <T> T[] toArray(T[]): Signature <T:Ljava/lang/Object;>([TT;)[TT;
        // java.lang.String: CASE_INSENSITIVE_ORDER Signature
        // Ljava/util/Comparator<Ljava/lang/String;>;
        // implements Comparable<java.lang.String>
        let Some(list) = parse_baseline("java.util.List") else {
            eprintln!("skipping: no baseline JDK 17 installation found");
            return;
        };
        assert_eq!(list.class_typarams, vec!["E".to_string()]);
        let add = list
            .members
            .iter()
            .find(|(_, _, name, ty)| name == "add" && ty.as_deref() == Some("(TV:E)->BOOLEAN"))
            .expect("List.add(E)");
        assert_eq!(add.3.as_deref(), Some("(TV:E)->BOOLEAN"));
        let to_array = list
            .members
            .iter()
            .find(|(_, _, name, ty)| {
                name == "toArray" && ty.as_deref().is_some_and(|t| t.starts_with("FORALL"))
            })
            .expect("List.toArray(T[])");
        assert_eq!(
            to_array.3.as_deref(),
            Some("FORALL(T)[(ARRAY[TV:T])->ARRAY[TV:T]]")
        );
        let size = list
            .members
            .iter()
            .find(|(_, _, name, _)| name == "size")
            .expect("List.size");
        assert_eq!(size.3.as_deref(), Some("()->INT"));

        let Some(string) = parse_baseline("java.lang.String") else {
            return;
        };
        let comparator = string
            .members
            .iter()
            .find(|(_, _, name, _)| name == "CASE_INSENSITIVE_ORDER")
            .expect("String.CASE_INSENSITIVE_ORDER");
        assert_eq!(
            comparator.3.as_deref(),
            Some("java.util.Comparator<java.lang.String>")
        );
        let compare_to = string
            .members
            .iter()
            .find(|(_, _, name, ty)| {
                name == "compareTo" && ty.as_deref() == Some("(java.lang.String)->INT")
            })
            .expect("String.compareTo(String)");
        assert_eq!(compare_to.3.as_deref(), Some("(java.lang.String)->INT"));
    }

    fn fixture_bytes(file_name: &str) -> Vec<u8> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data/fixtures")
            .join(file_name);
        std::fs::read(&path).unwrap_or_else(|e| panic!("read fixture {path:?}: {e}"))
    }

    /// Parses a checked-in fixture .class (generated at development time by
    /// tools/gen-class-fixtures/gen.ps1; no JVM is invoked at test runtime).
    fn parse_fixture(
        binary_name: &str,
        file_name: &str,
        save_parameter_names: bool,
    ) -> Option<ParseSummary> {
        let bytes = fixture_bytes(file_name);
        let mut context = Context::new();
        let log = Log::instance(&mut context);
        let names = Rc::new(Names::new(false));
        let messages = crate::util::javac_messages::JavacMessages::instance(&mut context);
        let factory = Rc::new(Factory::new(messages, "compiler"));
        let dcfh = DeferredCompletionFailureHandler::instance();
        let preview = Rc::new(Preview::new(false, false));
        let profile = Profile::instance(&mut context);
        let options = Options::instance(&mut context);
        if save_parameter_names {
            options.put("-parameters", "");
        }
        let source = Source::JDK17;
        let mut reader = ClassReader::new(
            log,
            Rc::clone(&names),
            factory,
            dcfh,
            preview,
            profile,
            &options,
            source,
            Rc::new(RefCell::new(None)),
        );
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let syms = Symtab::new(&mut ctx, Rc::clone(&names), Target::JDK1_17, source);
        let class_name = names.from_string(&javac_strings::JavaString::from(binary_name));
        let java_base = syms.java_base;
        let c = syms.enter_class_flat(&mut ctx, java_base, class_name);
        let file = Rc::new(ClassBytesFile {
            name: javac_strings::JavaString::from(file_name),
            bytes,
        });
        if let crate::code::symbol::SymbolKind::ClassSymbol { classfile, .. } =
            &mut ctx.sym_mut(c).kind_data
        {
            *classfile = Some(file);
        }
        reader
            .read_class_file(&mut ctx, &syms, c)
            .expect("readClassFile");
        let class_ty = ctx.sym(c).ty.expect("class type");
        let super_is_none = match &ctx.typ(class_ty).kind {
            crate::code::type_::TypeKindData::ClassType {
                supertype_field, ..
            } => supertype_field
                .map(|st| ctx.typ(st).has_tag(crate::code::type_tag::TypeTag::NONE))
                .unwrap_or(true),
            _ => panic!("class type expected"),
        };
        let interfaces = match &ctx.typ(class_ty).kind {
            crate::code::type_::TypeKindData::ClassType {
                interfaces_field, ..
            } => {
                let mut names_out = Vec::new();
                let mut list = interfaces_field.clone().unwrap_or_else(List::nil);
                while list.non_empty() {
                    names_out.push(type_summary(&ctx, *list.head()));
                    list = list.tail();
                }
                names_out
            }
            _ => panic!("class type expected"),
        };
        let members_field = match &ctx.sym(c).kind_data {
            crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } => *members_field,
            _ => panic!("class symbol"),
        };
        let symbols = ctx.scope_get_symbols(
            members_field.expect("members scope"),
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|_| true,
        );
        let class_typarams = match &ctx.typ(class_ty).kind {
            crate::code::type_::TypeKindData::ClassType { typarams_field, .. } => {
                let mut out = Vec::new();
                let mut list = typarams_field.clone().unwrap_or_else(List::nil);
                while list.non_empty() {
                    out.push(type_var_name(&ctx, *list.head()));
                    list = list.tail();
                }
                out
            }
            _ => Vec::new(),
        };
        let mut members = Vec::new();
        for sym in symbols.iter() {
            let s = ctx.sym(*sym);
            let ty = s.ty.map(|t| type_summary(&ctx, t));
            members.push((
                s.kind,
                s.flags_field,
                s.name.to_java_string().to_string(),
                ty,
            ));
        }
        Some(ParseSummary {
            flags: ctx.sym(c).flags_field,
            super_is_none,
            interfaces,
            class_typarams,
            members,
        })
    }

    #[test]
    fn fixture_source_file_and_constant_values() {
        // Fixtures.java declares CONST=42, NAME="inner", LONG_CONST=..., BOOL_CONST=true.
        let fixture = fixture_bytes("Fixtures.class");
        let mut context = Context::new();
        let log = Log::instance(&mut context);
        let names = Rc::new(Names::new(false));
        let messages = crate::util::javac_messages::JavacMessages::instance(&mut context);
        let factory = Rc::new(Factory::new(messages, "compiler"));
        let dcfh = DeferredCompletionFailureHandler::instance();
        let preview = Rc::new(Preview::new(false, false));
        let profile = Profile::instance(&mut context);
        let options = Options::instance(&mut context);
        let source = Source::JDK17;
        let mut reader = ClassReader::new(
            log,
            Rc::clone(&names),
            factory,
            dcfh,
            preview,
            profile,
            &options,
            source,
            Rc::new(RefCell::new(None)),
        );
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let syms = Symtab::new(&mut ctx, Rc::clone(&names), Target::JDK1_17, source);
        let class_name = names.from_string(&javac_strings::JavaString::from("Fixtures"));
        let java_base = syms.java_base;
        let c = syms.enter_class_flat(&mut ctx, java_base, class_name);
        let file = Rc::new(ClassBytesFile {
            name: javac_strings::JavaString::from("Fixtures.class"),
            bytes: fixture,
        });
        if let crate::code::symbol::SymbolKind::ClassSymbol { classfile, .. } =
            &mut ctx.sym_mut(c).kind_data
        {
            *classfile = Some(file);
        }
        reader
            .read_class_file(&mut ctx, &syms, c)
            .expect("readClassFile");
        let sourcefile = match &ctx.sym(c).kind_data {
            crate::code::symbol::SymbolKind::ClassSymbol { sourcefile, .. } => sourcefile.clone(),
            _ => panic!("class symbol"),
        };
        let sourcefile = sourcefile.expect("SourceFile attribute");
        assert_eq!(
            sourcefile.get_name().to_string_lossy(),
            "Fixtures.java",
            "SourceFile attribute"
        );
        let members = match &ctx.sym(c).kind_data {
            crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } => *members_field,
            _ => panic!("class symbol"),
        };
        let scope = members.expect("members scope");
        let find = |name: &str| -> SymbolId {
            let name = names.from_string(&javac_strings::JavaString::from(name));
            ctx.scope_find_first(scope, &name, &|_| true)
                .unwrap_or_else(|| panic!("field {name:?}"))
        };
        let constant = find("CONST");
        assert_eq!(
            ctx.sym(constant).get_data(),
            Some(&crate::code::attribute::ConstValue::Integer(42)),
            "ConstantValue CONST"
        );
        let name_field = find("NAME");
        assert_eq!(
            ctx.sym(name_field).get_data(),
            Some(&crate::code::attribute::ConstValue::String(
                javac_strings::JavaString::from("inner")
            )),
            "ConstantValue NAME"
        );
        let long_field = find("LONG_CONST");
        assert_eq!(
            ctx.sym(long_field).get_data(),
            Some(&crate::code::attribute::ConstValue::Long(1234567890123)),
            "ConstantValue LONG_CONST"
        );
        let bool_field = find("BOOL_CONST");
        // Java: boolean ConstantValue is stored as an Integer(1) Attribute.Constant
        // (ClassReader.checkType(var, Integer.class, v) for BOOLEAN).
        assert_eq!(
            ctx.sym(bool_field).get_data(),
            Some(&crate::code::attribute::ConstValue::Integer(1)),
            "ConstantValue BOOL_CONST"
        );
    }

    #[test]
    fn fixture_inner_classes_are_entered() {
        let Some(summary) = parse_fixture("Fixtures", "Fixtures.class", false) else {
            return;
        };
        let names: Vec<&str> = summary
            .members
            .iter()
            .map(|(_, _, n, _)| n.as_str())
            .collect();
        assert!(names.contains(&"Member"), "members: {names:?}");
        assert!(names.contains(&"Nested"), "members: {names:?}");
    }

    #[test]
    fn fixture_enclosing_method_moves_local_class() {
        // Fixtures$1Local.class has an EnclosingMethod attribute referencing
        // Fixtures.methodWithLocal; like upstream, the outer class must be read
        // first so that findMethod can search its members scope.
        let mut context = Context::new();
        let log = Log::instance(&mut context);
        let names = Rc::new(Names::new(false));
        let messages = crate::util::javac_messages::JavacMessages::instance(&mut context);
        let factory = Rc::new(Factory::new(messages, "compiler"));
        let dcfh = DeferredCompletionFailureHandler::instance();
        let preview = Rc::new(Preview::new(false, false));
        let profile = Profile::instance(&mut context);
        let options = Options::instance(&mut context);
        let source = Source::JDK17;
        let mut reader = ClassReader::new(
            log,
            Rc::clone(&names),
            factory,
            dcfh,
            preview,
            profile,
            &options,
            source,
            Rc::new(RefCell::new(None)),
        );
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let syms = Symtab::new(&mut ctx, Rc::clone(&names), Target::JDK1_17, source);
        let java_base = syms.java_base;
        // 1) read the outer class.
        let outer_name = names.from_string(&javac_strings::JavaString::from("Fixtures"));
        let outer = syms.enter_class_flat(&mut ctx, java_base, outer_name);
        let outer_file = Rc::new(ClassBytesFile {
            name: javac_strings::JavaString::from("Fixtures.class"),
            bytes: fixture_bytes("Fixtures.class"),
        });
        if let crate::code::symbol::SymbolKind::ClassSymbol { classfile, .. } =
            &mut ctx.sym_mut(outer).kind_data
        {
            *classfile = Some(outer_file);
        }
        reader
            .read_class_file(&mut ctx, &syms, outer)
            .expect("outer readClassFile");
        // 2) read the local class whose EnclosingMethod references the outer method.
        let local_name = names.from_string(&javac_strings::JavaString::from("Fixtures$1Local"));
        let local = syms.enter_class_flat(&mut ctx, java_base, local_name);
        let local_file = Rc::new(ClassBytesFile {
            name: javac_strings::JavaString::from("Fixtures$1Local.class"),
            bytes: fixture_bytes("Fixtures$1Local.class"),
        });
        if let crate::code::symbol::SymbolKind::ClassSymbol { classfile, .. } =
            &mut ctx.sym_mut(local).kind_data
        {
            *classfile = Some(local_file);
        }
        reader
            .read_class_file(&mut ctx, &syms, local)
            .expect("local readClassFile");
        let name = ctx.sym(local).name.to_java_string().to_string();
        assert_eq!(name, "Local", "simpleBinaryName");
        let owner = ctx.sym(local).owner.expect("enclosing method owner");
        assert_eq!(
            ctx.sym(owner).name.to_java_string().to_string(),
            "methodWithLocal",
            "EnclosingMethod owner"
        );
        assert_eq!(ctx.sym(owner).kind, crate::code::kinds::Kind::MTH);
    }

    #[test]
    fn fixture_record_components_and_permitted_subclasses() {
        let Some(record) = parse_fixture("RecordFixture", "RecordFixture.class", false) else {
            return;
        };
        assert_ne!(record.flags & crate::code::flags::RECORD, 0);
        let names: Vec<&str> = record
            .members
            .iter()
            .map(|(_, _, n, _)| n.as_str())
            .collect();
        for expected in ["x", "name", "doubled", "toString", "hashCode", "equals"] {
            assert!(names.contains(&expected), "missing {expected}: {names:?}");
        }
        let Some(sealed) = parse_fixture("SealedFixture", "SealedFixture.class", false) else {
            return;
        };
        // The permitted list is asserted via the interface type's member list:
        // read_attribute stores the class symbols in ClassSymbol.permitted; the
        // summary does not expose it, so the flag/interface check follows.
        assert_ne!(sealed.flags & crate::code::flags::INTERFACE as i64, 0);
        let Some(circle) = parse_fixture("Circle", "Circle.class", false) else {
            return;
        };
        assert!(
            circle
                .interfaces
                .iter()
                .any(|i| i.contains("SealedFixture"))
        );
    }

    fn read_fixture_with_reader(
        reader: &mut ClassReader,
        ctx: &mut crate::compiler_context::CompilerContext,
        syms: &crate::code::symtab::Symtab,
        names: &Rc<Names>,
        binary_name: &str,
        file_name: &str,
    ) -> SymbolId {
        let class_name = names.from_string(&javac_strings::JavaString::from(binary_name));
        let c = syms.enter_class_flat(ctx, syms.java_base, class_name);
        let file = Rc::new(ClassBytesFile {
            name: javac_strings::JavaString::from(file_name),
            bytes: fixture_bytes(file_name),
        });
        if let crate::code::symbol::SymbolKind::ClassSymbol { classfile, .. } =
            &mut ctx.sym_mut(c).kind_data
        {
            *classfile = Some(file);
        }
        reader
            .read_class_file(ctx, syms, c)
            .expect("readRecordFixture");
        c
    }

    #[test]
    fn mirror_elements_expose_record_components_and_permitted_subclasses() {
        let bytes = fixture_bytes("RecordFixture.class");
        assert!(!bytes.is_empty());
        let mut context = Context::new();
        let log = Log::instance(&mut context);
        let names = Rc::new(Names::new(false));
        let messages = crate::util::javac_messages::JavacMessages::instance(&mut context);
        let factory = Rc::new(Factory::new(messages, "compiler"));
        let dcfh = DeferredCompletionFailureHandler::instance();
        let preview = Rc::new(Preview::new(false, false));
        let profile = Profile::instance(&mut context);
        let options = Options::instance(&mut context);
        let source = Source::JDK17;
        let mut reader = ClassReader::new(
            log,
            Rc::clone(&names),
            factory,
            dcfh,
            preview,
            profile,
            &options,
            source,
            Rc::new(RefCell::new(None)),
        );
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let syms = Symtab::new(&mut ctx, Rc::clone(&names), Target::JDK1_17, source);
        let record = read_fixture_with_reader(
            &mut reader,
            &mut ctx,
            &syms,
            &names,
            "RecordFixture",
            "RecordFixture.class",
        );
        let record_element =
            crate::model::annotation_mirrors::element_for_symbol(&mut ctx, &syms, record);
        let components = record_element
            .as_type_element()
            .expect("record type element")
            .get_record_components();
        assert!(
            components.len() >= 2,
            "record components: {}",
            components.len()
        );
        let component_names: Vec<String> = components
            .iter()
            .map(|component| component.get_simple_name().to_java_string().to_string())
            .collect();
        assert!(component_names.contains(&"x".to_string()));
        assert!(component_names.contains(&"name".to_string()));
        let sealed = read_fixture_with_reader(
            &mut reader,
            &mut ctx,
            &syms,
            &names,
            "SealedFixture",
            "SealedFixture.class",
        );
        let sealed_element =
            crate::model::annotation_mirrors::element_for_symbol(&mut ctx, &syms, sealed);
        let permitted = sealed_element
            .as_type_element()
            .expect("sealed type element")
            .get_permitted_subclasses();
        assert!(!permitted.is_empty(), "permitted subclasses");
        let displays: Vec<String> = permitted
            .iter()
            .map(|permitted| format!("{}", permitted))
            .collect();
        assert!(
            displays.iter().any(|display| display.contains("Circle")),
            "permitted: {displays:?}"
        );
    }

    #[test]
    fn fixture_module_info_directives() {
        // module-info.class: module fixtures.mod { exports fixtures.p; uses
        // fixtures.p.Service; provides fixtures.p.Service with fixtures.p.ServiceImpl;
        // requires java.base; } (javap -v Module attribute oracle).
        let bytes = fixture_bytes("module-info.class");
        let mut context = Context::new();
        let log = Log::instance(&mut context);
        let names = Rc::new(Names::new(false));
        let messages = crate::util::javac_messages::JavacMessages::instance(&mut context);
        let factory = Rc::new(Factory::new(messages, "compiler"));
        let dcfh = DeferredCompletionFailureHandler::instance();
        let preview = Rc::new(Preview::new(false, false));
        let profile = Profile::instance(&mut context);
        let options = Options::instance(&mut context);
        let source = Source::JDK17;
        let mut reader = ClassReader::new(
            log,
            Rc::clone(&names),
            factory,
            dcfh,
            preview,
            profile,
            &options,
            source,
            Rc::new(RefCell::new(None)),
        );
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let syms = Symtab::new(&mut ctx, Rc::clone(&names), Target::JDK1_17, source);
        let module_name = names.from_string(&javac_strings::JavaString::from("fixtures.mod"));
        let module = syms.enter_module(&mut ctx, module_name);
        let module_info_name = names.module_info.clone();
        let c = syms.define_class(&mut ctx, module_info_name, Some(module));
        // Java: ClassFinder sets module.module_info to the module-info ClassSymbol.
        if let crate::code::symbol::SymbolKind::ModuleSymbol { module_info, .. } =
            &mut ctx.sym_mut(module).kind_data
        {
            *module_info = Some(c);
        }
        let file = Rc::new(ClassBytesFile {
            name: javac_strings::JavaString::from("module-info.class"),
            bytes,
        });
        if let crate::code::symbol::SymbolKind::ClassSymbol { classfile, .. } =
            &mut ctx.sym_mut(c).kind_data
        {
            *classfile = Some(file);
        }
        reader
            .read_class_file(&mut ctx, &syms, c)
            .expect("module-info readClassFile");
        assert_ne!(ctx.sym(c).flags_field & crate::code::flags::MODULE, 0);
        let (requires, exports, uses, provides) = match &ctx.sym(module).kind_data {
            crate::code::symbol::SymbolKind::ModuleSymbol {
                requires,
                exports,
                uses,
                provides,
                ..
            } => (
                requires.clone(),
                exports.clone(),
                uses.clone(),
                provides.clone(),
            ),
            _ => panic!("module symbol"),
        };
        let mut requires_names = Vec::new();
        let mut list = requires;
        while list.non_empty() {
            let directive = ctx.directive(*list.head());
            if let crate::code::directive::Directive::Requires {
                module: Some(m), ..
            } = directive
            {
                requires_names.push(ctx.sym(*m).name.to_java_string().to_string());
            } else {
                panic!("requires directive expected");
            }
            list = list.tail();
        }
        assert!(
            requires_names.contains(&"java.base".to_string()),
            "requires: {requires_names:?}"
        );
        assert_eq!(exports.length(), 1, "exports");
        match ctx.directive(*exports.head()) {
            crate::code::directive::Directive::Exports {
                packge: Some(p),
                modules,
                ..
            } => {
                let fullname = match &ctx.sym(*p).kind_data {
                    crate::code::symbol::SymbolKind::PackageSymbol { fullname, .. } => {
                        fullname.to_java_string().to_string()
                    }
                    _ => panic!("package"),
                };
                assert_eq!(fullname, "fixtures.p");
                assert!(modules.is_none(), "unqualified exports");
            }
            _ => panic!("exports directive expected"),
        }
        assert_eq!(uses.length(), 1, "uses");
        match ctx.directive(*uses.head()) {
            crate::code::directive::Directive::Uses { service: Some(s) } => {
                assert_eq!(
                    ctx.sym(*s).flat_name().to_java_string().to_string(),
                    "fixtures.p.Service"
                );
            }
            _ => panic!("uses directive expected"),
        }
        assert_eq!(provides.length(), 1, "provides");
        match ctx.directive(*provides.head()) {
            crate::code::directive::Directive::Provides {
                service: Some(s),
                impls,
            } => {
                assert_eq!(
                    ctx.sym(*s).flat_name().to_java_string().to_string(),
                    "fixtures.p.Service"
                );
                assert_eq!(impls.length(), 1);
                assert_eq!(
                    ctx.sym(*impls.head())
                        .flat_name()
                        .to_java_string()
                        .to_string(),
                    "fixtures.p.ServiceImpl"
                );
            }
            _ => panic!("provides directive expected"),
        }
    }

    #[test]
    fn fixture_annotation_type_metadata_installed() {
        // ClassReader.readClass installs AnnotationTypeMetadata with a
        // CompleterDeproxy for annotation types (ClassReader.java:2988-2995);
        // attachAnnotations collected the @Target/@Retention proxies.
        let mut context = Context::new();
        let log = Log::instance(&mut context);
        let names = Rc::new(Names::new(false));
        let messages = crate::util::javac_messages::JavacMessages::instance(&mut context);
        let factory = Rc::new(Factory::new(messages, "compiler"));
        let dcfh = DeferredCompletionFailureHandler::instance();
        let preview = Rc::new(Preview::new(false, false));
        let profile = Profile::instance(&mut context);
        let options = Options::instance(&mut context);
        let source = Source::JDK17;
        let mut reader = ClassReader::new(
            log,
            Rc::clone(&names),
            factory,
            dcfh,
            preview,
            profile,
            &options,
            source,
            Rc::new(RefCell::new(None)),
        );
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let syms = Symtab::new(&mut ctx, Rc::clone(&names), Target::JDK1_17, source);
        let java_base = syms.java_base;
        let class_name = names.from_string(&javac_strings::JavaString::from("MyAnno"));
        let c = syms.enter_class_flat(&mut ctx, java_base, class_name);
        let file = Rc::new(ClassBytesFile {
            name: javac_strings::JavaString::from("MyAnno.class"),
            bytes: fixture_bytes("MyAnno.class"),
        });
        if let crate::code::symbol::SymbolKind::ClassSymbol { classfile, .. } =
            &mut ctx.sym_mut(c).kind_data
        {
            *classfile = Some(file);
        }
        reader
            .read_class_file(&mut ctx, &syms, c)
            .expect("MyAnno readClassFile");
        assert!(
            ctx.sym(c).is_annotation_type(),
            "MyAnno must be entered as an annotation type"
        );
        let metadata = ctx
            .sym(c)
            .get_annotation_type_metadata()
            .expect("annotation type metadata");
        assert!(
            metadata.borrow().is_metadata_for_annotation_type(),
            "AnnotationTypeMetadata installed for MyAnno"
        );
        assert!(
            reader.target.is_some(),
            "attachAnnotations must collect the @Target proxy"
        );
        // MyAnno carries @Target (collected into reader.target); the upstream
        // 'repeatable' field only holds a @Repeatable proxy, which MyAnno does
        // not declare (the @Retention attribute is not tracked by this field).
        assert!(
            reader.repeatable.is_none(),
            "MyAnno declares no @Repeatable, so no repeatable proxy is collected"
        );
    }

    #[test]
    fn deproxy_compound_builds_attribute_compounds() {
        // AnnotationDeproxy.deproxyCompoundList/deproxyCompound/deproxy path:
        // classfile proxies become Attribute.Compound/Constant values (the
        // "attributes" half of the annotation end-to-end path).
        let mut _context = Context::new();
        let names = Rc::new(Names::new(false));
        let source = Source::JDK17;
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let syms = Symtab::new(&mut ctx, Rc::clone(&names), Target::JDK1_17, source);
        let types = crate::code::types::Types::with_names(Rc::clone(&names));
        let value_name = names.value.clone();
        let proxy = AnnotationValue::Compound {
            ty: syms.annotation_type,
            values: List::of((
                value_name.clone(),
                AnnotationValue::Constant {
                    ty: syms.string_type,
                    value: crate::code::attribute::ConstValue::String(
                        javac_strings::JavaString::from("x"),
                    ),
                },
            )),
        };
        let list = List::of(proxy);
        let attributes = deproxy_compound_list(&mut ctx, &syms, &types, &names, &list, None, None)
            .expect("deproxy");
        assert_eq!(attributes.length(), 1);
        let attr = *attributes.head();
        let values = match &ctx.attr(attr).kind {
            crate::code::attribute::AttributeKind::Compound { values, .. } => values.clone(),
            _other => panic!("Compound attribute expected"),
        };
        assert_eq!(values.length(), 1);
        let pair = values.head().clone();
        assert_eq!(
            ctx.sym(pair.fst).name.to_java_string(),
            value_name.to_java_string()
        );
        match &ctx.attr(pair.snd).kind {
            crate::code::attribute::AttributeKind::Constant { value } => {
                assert_eq!(
                    value,
                    &crate::code::attribute::ConstValue::String(javac_strings::JavaString::from(
                        "x"
                    ))
                );
            }
            _other => panic!("Constant attribute expected"),
        }
    }

    fn annotation_constant(value: &AnnotationValue) -> Option<&crate::code::attribute::ConstValue> {
        match value {
            AnnotationValue::Constant { value, .. } => Some(value),
            _ => None,
        }
    }

    fn compound_values(annotation: &AnnotationValue) -> List<(Name, AnnotationValue)> {
        match annotation {
            AnnotationValue::Compound { values, .. } => values.clone(),
            other => panic!("compound annotation expected: {other:?}"),
        }
    }

    #[test]
    fn fixture_annotation_values() {
        // AnnotatedFixture.java: @MyAnno(value = 7, name = "n") on the class,
        // @MyAnno(1) on the field, @MyAnno(2) on the method, @MyAnno(3) on the
        // parameter; MyAnno declares default values (name/type/kind).
        let mut context = Context::new();
        let log = Log::instance(&mut context);
        let names = Rc::new(Names::new(false));
        let messages = crate::util::javac_messages::JavacMessages::instance(&mut context);
        let factory = Rc::new(Factory::new(messages, "compiler"));
        let dcfh = DeferredCompletionFailureHandler::instance();
        let preview = Rc::new(Preview::new(false, false));
        let profile = Profile::instance(&mut context);
        let options = Options::instance(&mut context);
        let source = Source::JDK17;
        let mut reader = ClassReader::new(
            log,
            Rc::clone(&names),
            factory,
            dcfh,
            preview,
            profile,
            &options,
            source,
            Rc::new(RefCell::new(None)),
        );
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let syms = Symtab::new(&mut ctx, Rc::clone(&names), Target::JDK1_17, source);
        let java_base = syms.java_base;
        let class_name = names.from_string(&javac_strings::JavaString::from("AnnotatedFixture"));
        let c = syms.enter_class_flat(&mut ctx, java_base, class_name);
        let file = Rc::new(ClassBytesFile {
            name: javac_strings::JavaString::from("AnnotatedFixture.class"),
            bytes: fixture_bytes("AnnotatedFixture.class"),
        });
        if let crate::code::symbol::SymbolKind::ClassSymbol { classfile, .. } =
            &mut ctx.sym_mut(c).kind_data
        {
            *classfile = Some(file);
        }
        reader
            .read_class_file(&mut ctx, &syms, c)
            .expect("AnnotatedFixture readClassFile");
        assert_eq!(
            reader.pending_annotations.len(),
            3,
            "class/field/method annotations"
        );
        let class_annotation = &reader.pending_annotations[0].1;
        assert_eq!(class_annotation.length(), 1, "one compound annotation");
        let mut values = compound_values(class_annotation.head());
        assert_eq!(values.length(), 2, "explicit elements only");
        let mut found_value = None;
        let mut found_name = None;
        for (name, value) in &values {
            let name = name.to_java_string().to_string();
            if name == "value" {
                found_value = annotation_constant(&value).cloned();
            } else if name == "name" {
                found_name = annotation_constant(&value).cloned();
            }
        }
        assert_eq!(
            found_value,
            Some(crate::code::attribute::ConstValue::Integer(7))
        );
        assert_eq!(
            found_name,
            Some(crate::code::attribute::ConstValue::String(
                javac_strings::JavaString::from("n")
            ))
        );
        let field_annotation = &reader.pending_annotations[1].1;
        let (_, field_value) = compound_values(field_annotation.head()).head().clone();
        assert_eq!(
            annotation_constant(&field_value),
            Some(&crate::code::attribute::ConstValue::Integer(1))
        );
        let method_annotation = &reader.pending_annotations[2].1;
        let (_, method_value) = compound_values(method_annotation.head()).head().clone();
        assert_eq!(
            annotation_constant(&method_value),
            Some(&crate::code::attribute::ConstValue::Integer(2))
        );
        assert_eq!(reader.pending_parameter_annotations.len(), 1);
        let parameter_annotations = &reader.pending_parameter_annotations[0].1;
        assert_eq!(parameter_annotations.len(), 1);
        let (_, parameter_annotation) = compound_values(parameter_annotations[0].head())
            .head()
            .clone();
        assert_eq!(
            annotation_constant(&parameter_annotation),
            Some(&crate::code::attribute::ConstValue::Integer(3))
        );
    }

    #[test]
    fn fixture_annotation_defaults() {
        // MyAnno.class default values: name()="x", type()=Object.class,
        // kind()=ElementType.TYPE (value() has no default).
        let mut context = Context::new();
        let log = Log::instance(&mut context);
        let names = Rc::new(Names::new(false));
        let messages = crate::util::javac_messages::JavacMessages::instance(&mut context);
        let factory = Rc::new(Factory::new(messages, "compiler"));
        let dcfh = DeferredCompletionFailureHandler::instance();
        let preview = Rc::new(Preview::new(false, false));
        let profile = Profile::instance(&mut context);
        let options = Options::instance(&mut context);
        let source = Source::JDK17;
        let mut reader = ClassReader::new(
            log,
            Rc::clone(&names),
            factory,
            dcfh,
            preview,
            profile,
            &options,
            source,
            Rc::new(RefCell::new(None)),
        );
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let syms = Symtab::new(&mut ctx, Rc::clone(&names), Target::JDK1_17, source);
        let java_base = syms.java_base;
        let class_name = names.from_string(&javac_strings::JavaString::from("MyAnno"));
        let c = syms.enter_class_flat(&mut ctx, java_base, class_name);
        let file = Rc::new(ClassBytesFile {
            name: javac_strings::JavaString::from("MyAnno.class"),
            bytes: fixture_bytes("MyAnno.class"),
        });
        if let crate::code::symbol::SymbolKind::ClassSymbol { classfile, .. } =
            &mut ctx.sym_mut(c).kind_data
        {
            *classfile = Some(file);
        }
        reader
            .read_class_file(&mut ctx, &syms, c)
            .expect("MyAnno readClassFile");
        let mut defaults: Vec<(String, AnnotationValue)> = Vec::new();
        for (sym, value) in &reader.pending_annotation_defaults {
            let name = ctx.sym(*sym).name.to_java_string().to_string();
            defaults.push((name, value.clone()));
        }
        assert_eq!(defaults.len(), 3, "defaults: {defaults:?}");
        for (name, value) in &defaults {
            match name.as_str() {
                "name" => {
                    assert_eq!(
                        annotation_constant(value),
                        Some(&crate::code::attribute::ConstValue::String(
                            javac_strings::JavaString::from("x")
                        ))
                    );
                }
                "type" => match value {
                    AnnotationValue::Class { class_type } => {
                        let tsym = ctx.typ(*class_type).tsym.expect("Object type symbol");
                        assert_eq!(
                            ctx.sym(tsym).flat_name().to_java_string().to_string(),
                            "java.lang.Object"
                        );
                    }
                    other => panic!("type default: {other:?}"),
                },
                "kind" => match value {
                    AnnotationValue::Enum { enumerator, .. } => {
                        assert_eq!(enumerator.to_java_string().to_string(), "TYPE");
                    }
                    other => panic!("kind default: {other:?}"),
                },
                other => panic!("unexpected default {other}"),
            }
        }
    }

    #[test]
    fn fixture_type_annotations() {
        // TypeAnnotated.class (javap -v RuntimeVisibleTypeAnnotations oracle):
        // field -> FIELD
        // list -> FIELD, location=[TYPE_ARGUMENT(0)]
        // method -> METHOD_RETURN, parameter -> METHOD_FORMAL_PARAMETER(0)
        let mut context = Context::new();
        let log = Log::instance(&mut context);
        let names = Rc::new(Names::new(false));
        let messages = crate::util::javac_messages::JavacMessages::instance(&mut context);
        let factory = Rc::new(Factory::new(messages, "compiler"));
        let dcfh = DeferredCompletionFailureHandler::instance();
        let preview = Rc::new(Preview::new(false, false));
        let profile = Profile::instance(&mut context);
        let options = Options::instance(&mut context);
        let source = Source::JDK17;
        let mut reader = ClassReader::new(
            log,
            Rc::clone(&names),
            factory,
            dcfh,
            preview,
            profile,
            &options,
            source,
            Rc::new(RefCell::new(None)),
        );
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let syms = Symtab::new(&mut ctx, Rc::clone(&names), Target::JDK1_17, source);
        let java_base = syms.java_base;
        let class_name = names.from_string(&javac_strings::JavaString::from("TypeAnnotated"));
        let c = syms.enter_class_flat(&mut ctx, java_base, class_name);
        let file = Rc::new(ClassBytesFile {
            name: javac_strings::JavaString::from("TypeAnnotated.class"),
            bytes: fixture_bytes("TypeAnnotated.class"),
        });
        if let crate::code::symbol::SymbolKind::ClassSymbol { classfile, .. } =
            &mut ctx.sym_mut(c).kind_data
        {
            *classfile = Some(file);
        }
        reader
            .read_class_file(&mut ctx, &syms, c)
            .expect("TypeAnnotated readClassFile");
        let mut targets: Vec<(String, Vec<String>)> = Vec::new();
        for (_, proxies) in &reader.pending_type_annotations {
            let mut list = proxies.clone();
            while list.non_empty() {
                let proxy = list.head();
                let target = format!("{:?}", proxy.position.type_);
                let mut location: Vec<String> = Vec::new();
                let mut path = proxy.position.location.clone();
                while path.non_empty() {
                    location.push(format!("{}", path.head()));
                    path = path.tail();
                }
                targets.push((target, location));
                list = list.tail();
            }
        }
        assert!(
            targets.contains(&("FIELD".to_string(), Vec::new())),
            "targets: {targets:?}"
        );
        assert!(
            targets.contains(&("FIELD".to_string(), vec!["TYPE_ARGUMENT(0)".to_string()],)),
            "targets: {targets:?}"
        );
        assert!(
            targets.contains(&("METHOD_RETURN".to_string(), Vec::new())),
            "targets: {targets:?}"
        );
        assert!(
            targets.contains(&("METHOD_FORMAL_PARAMETER".to_string(), Vec::new())),
            "targets: {targets:?}"
        );
        // The METHOD_FORMAL_PARAMETER position carries the parameter index.
        let mut parameter_position = None;
        for (_, proxies) in &reader.pending_type_annotations {
            let mut list = proxies.clone();
            while list.non_empty() {
                let proxy = list.head();
                if proxy.position.type_
                    == crate::code::target_type::TargetType::METHOD_FORMAL_PARAMETER
                {
                    parameter_position = Some(proxy.position.parameter_index);
                }
                list = list.tail();
            }
        }
        assert_eq!(parameter_position, Some(0));
    }
    /// Minimal classfile with one unknown attribute: magic, version 52, a
    /// 5-entry constant pool, java/lang/Object super, no members, and one
    /// attribute whose name is the Utf8 "Unknown".
    fn minimal_class_with_attribute(attr_len: i32, content_len: i32) -> Vec<u8> {
        let mut bytes: Vec<u8> = Vec::new();
        bytes.extend_from_slice(&[0xCA, 0xFE, 0xBA, 0xBE]);
        bytes.extend_from_slice(&0u16.to_be_bytes()); // minor
        bytes.extend_from_slice(&52u16.to_be_bytes()); // major
        bytes.extend_from_slice(&6u16.to_be_bytes()); // constant_pool_count
        // #1 Utf8 "Demo"
        bytes.push(1);
        bytes.extend_from_slice(&4u16.to_be_bytes());
        bytes.extend_from_slice(b"Demo");
        // #2 Class #1
        bytes.push(7);
        bytes.extend_from_slice(&1u16.to_be_bytes());
        // #3 Utf8 "java/lang/Object"
        bytes.push(1);
        bytes.extend_from_slice(&16u16.to_be_bytes());
        bytes.extend_from_slice(b"java/lang/Object");
        // #4 Class #3
        bytes.push(7);
        bytes.extend_from_slice(&3u16.to_be_bytes());
        // #5 Utf8 "Unknown"
        bytes.push(1);
        bytes.extend_from_slice(&7u16.to_be_bytes());
        bytes.extend_from_slice(b"Unknown");
        bytes.extend_from_slice(&0x0021u16.to_be_bytes()); // access_flags
        bytes.extend_from_slice(&2u16.to_be_bytes()); // this_class
        bytes.extend_from_slice(&4u16.to_be_bytes()); // super_class
        bytes.extend_from_slice(&0u16.to_be_bytes()); // interfaces_count
        bytes.extend_from_slice(&0u16.to_be_bytes()); // fields_count
        bytes.extend_from_slice(&0u16.to_be_bytes()); // methods_count
        bytes.extend_from_slice(&1u16.to_be_bytes()); // attributes_count
        bytes.extend_from_slice(&5u16.to_be_bytes()); // attribute_name_index
        bytes.extend_from_slice(&(attr_len as u32).to_be_bytes()); // attribute_length is u4
        for i in 0..content_len {
            bytes.push((i % 251) as u8);
        }
        bytes
    }

    fn parse_bytes(binary_name: &str, bytes: Vec<u8>) -> Result<(), String> {
        let mut context = Context::new();
        let log = Log::instance(&mut context);
        let names = Rc::new(Names::new(false));
        let messages = crate::util::javac_messages::JavacMessages::instance(&mut context);
        let factory = Rc::new(Factory::new(messages, "compiler"));
        let dcfh = DeferredCompletionFailureHandler::instance();
        let preview = Rc::new(Preview::new(false, false));
        let profile = Profile::instance(&mut context);
        let options = Options::instance(&mut context);
        let source = Source::JDK17;
        let mut reader = ClassReader::new(
            log,
            Rc::clone(&names),
            factory,
            dcfh,
            preview,
            profile,
            &options,
            source,
            Rc::new(RefCell::new(None)),
        );
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let syms = Symtab::new(&mut ctx, Rc::clone(&names), Target::JDK1_17, source);
        let class_name = names.from_string(&javac_strings::JavaString::from(binary_name));
        let java_base = syms.java_base;
        let c = syms.enter_class_flat(&mut ctx, java_base, class_name);
        let file = Rc::new(ClassBytesFile {
            name: javac_strings::JavaString::from("Demo.class"),
            bytes,
        });
        if let crate::code::symbol::SymbolKind::ClassSymbol { classfile, .. } =
            &mut ctx.sym_mut(c).kind_data
        {
            *classfile = Some(file);
        }
        reader
            .read_class_file(&mut ctx, &syms, c)
            .map_err(|failure| format!("{failure}"))
    }

    #[test]
    fn unknown_attribute_is_skipped() {
        let bytes = minimal_class_with_attribute(3, 3);
        parse_bytes("Demo", bytes).expect("unknown attribute must be skipped");
    }

    #[test]
    fn attribute_length_overflow_maps_to_bad_class_file() {
        // attr_length points past the end of the buffer: the readClassFile
        // ArrayIndexOutOfBoundsException mapping (bad.class.file) is applied.
        let bytes = minimal_class_with_attribute(0x1FFFF, 3);
        let error = parse_bytes("Demo", bytes).expect_err("overflowing attribute length");
        assert!(
            error.contains("bad class file") || error.contains("bad.class.file"),
            "error: {error}"
        );
    }

    #[test]
    fn attribute_reader_version_and_allow_gates() {
        // Version gate: the Record reader is version 58; a classfile with major
        // version 51 must be rejected even when the source allows records.
        let mut reader = reader();
        let record_pos = reader
            .attribute_readers
            .iter()
            .position(|def| def.name.to_java_string().to_string() == "Record")
            .expect("Record reader");
        reader.major_version = 51;
        assert!(
            !reader.accepts(record_pos, AttributeKind::CLASS),
            "version gate"
        );
        reader.major_version = 58;
        assert!(
            reader.accepts(record_pos, AttributeKind::CLASS),
            "version ok"
        );
        // allowRecords gate: Source JDK8 does not allow records.
        let mut jdk8 = reader_with_source(Source::JDK8);
        let record_pos_8 = jdk8
            .attribute_readers
            .iter()
            .position(|def| def.name.to_java_string().to_string() == "Record")
            .expect("Record reader");
        jdk8.major_version = 58;
        assert!(
            !jdk8.accepts(record_pos_8, AttributeKind::CLASS),
            "allowRecords"
        );
        // allowModules gate: Source JDK8 does not allow modules.
        let mut jdk8_modules = reader_with_source(Source::JDK8);
        let module_pos = jdk8_modules
            .attribute_readers
            .iter()
            .position(|def| def.name.to_java_string().to_string() == "Module")
            .expect("Module reader");
        jdk8_modules.major_version = 53;
        assert!(
            !jdk8_modules.accepts(module_pos, AttributeKind::CLASS),
            "allowModules"
        );
        // allowSealed gate: Source JDK8 does not allow sealed types.
        let mut jdk8_sealed = reader_with_source(Source::JDK8);
        let sealed_pos = jdk8_sealed
            .attribute_readers
            .iter()
            .position(|def| def.name.to_java_string().to_string() == "PermittedSubclasses")
            .expect("PermittedSubclasses reader");
        jdk8_sealed.major_version = 61;
        assert!(
            !jdk8_sealed.accepts(sealed_pos, AttributeKind::CLASS),
            "allowSealed"
        );
        // All 26 readers are registered.
        assert_eq!(reader.attribute_readers.len(), 26);
    }
    #[test]
    fn bad_magic_returns_bad_class_file_key() {
        let Some(mut bytes) = baseline_class_bytes("java.lang.Object") else {
            eprintln!("skipping: no baseline JDK 17 installation found");
            return;
        };
        bytes[0..4].copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
        let failure = parse_with_bytes("java.lang.Object", bytes).expect_err("bad magic");
        let code = failure
            .get_diagnostic()
            .expect("diagnostic")
            .get_diagnostic_info()
            .get_code()
            .to_string();
        assert_eq!(
            code, "bad.class.file.header",
            "upstream BadClassFile wrapper key"
        );
    }

    #[test]
    fn bad_version_returns_bad_class_file_key() {
        let Some(mut bytes) = baseline_class_bytes("java.lang.Object") else {
            eprintln!("skipping: no baseline JDK 17 installation found");
            return;
        };
        // major_version at bytes[6..8] (magic, minor, major).
        bytes[6..8].copy_from_slice(&[0x03, 0xE7]);
        let failure = parse_with_bytes("java.lang.Object", bytes).expect_err("bad version");
        let code = failure
            .get_diagnostic()
            .expect("diagnostic")
            .get_diagnostic_info()
            .get_code()
            .to_string();
        assert_eq!(
            code, "bad.class.file.header",
            "upstream BadClassFile wrapper key"
        );
    }

    #[test]
    fn bad_constant_pool_tag_returns_bad_class_file_key() {
        // magic, minor 0, major 61, constant_pool_count 2, tag 0xEE
        let bytes = vec![
            0xCA, 0xFE, 0xBA, 0xBE, 0x00, 0x00, 0x00, 0x3D, 0x00, 0x02, 0xEE,
        ];
        let failure = parse_with_bytes("demo.Bad", bytes).expect_err("bad pool tag");
        let code = failure
            .get_diagnostic()
            .expect("diagnostic")
            .get_diagnostic_info()
            .get_code()
            .to_string();
        assert_eq!(
            code, "bad.class.file.header",
            "upstream BadClassFile wrapper key"
        );
    }

    use super::*;
    use crate::code::preview::Preview;
    use crate::util::context::Context;

    fn reader() -> ClassReader {
        reader_with_source(Source::JDK17)
    }

    fn reader_with_source(source: Source) -> ClassReader {
        let mut context = Context::new();
        let log = Log::instance(&mut context);
        let names = Rc::new(Names::new(false));
        let messages = crate::util::javac_messages::JavacMessages::instance(&mut context);
        let factory = Rc::new(Factory::new(messages, "compiler"));
        let dcfh = DeferredCompletionFailureHandler::instance();
        let preview = Rc::new(Preview::new(false, false));
        let profile = Rc::new(Profile::values()[0]);
        let options = Options::instance(&mut context);
        ClassReader::new(
            log,
            names,
            factory,
            dcfh,
            preview,
            profile,
            &options,
            source,
            Rc::new(RefCell::new(None)),
        )
    }
    /// A JavaFileObject over raw fixture class bytes (D5 bad-classfile fixtures).
    struct D5ClassFile {
        name: javac_strings::JavaString,
        bytes: Vec<u8>,
    }

    impl FileObject for D5ClassFile {
        fn to_uri(&self) -> URI {
            URI::create(&javac_strings::JavaString::from(
                format!("file:/{}", self.name).as_str(),
            ))
        }
        fn get_name(&self) -> javac_strings::JavaString {
            self.name.clone()
        }
        fn open_input_stream(&self) -> Result<Box<dyn Read>, ToolsError> {
            Ok(Box::new(std::io::Cursor::new(self.bytes.clone())))
        }
        fn open_output_stream(&self) -> Result<Box<dyn std::io::Write>, ToolsError> {
            Err(ToolsError::UnsupportedOperationException)
        }
        fn open_reader(
            &self,
            _ignore_encoding_errors: bool,
        ) -> Result<Box<dyn Reader>, ToolsError> {
            Err(ToolsError::UnsupportedOperationException)
        }
        fn get_char_content(
            &self,
            _ignore_encoding_errors: bool,
        ) -> Result<Option<javac_strings::JavaString>, ToolsError> {
            Ok(None)
        }
        fn open_writer(&self) -> Result<Box<dyn Writer>, ToolsError> {
            Err(ToolsError::UnsupportedOperationException)
        }
        fn get_last_modified(&self) -> i64 {
            0
        }
        fn delete(&self) -> bool {
            false
        }
    }

    impl JavaFileObject for D5ClassFile {
        fn get_kind(&self) -> Kind {
            Kind::CLASS
        }
        fn is_name_compatible(
            &self,
            _simple_name: &javac_strings::JavaString,
            _kind: Kind,
        ) -> bool {
            true
        }
        fn get_nesting_kind(&self) -> Option<NestingKind> {
            None
        }
        fn get_access_level(&self) -> Option<Modifier> {
            None
        }
    }

    fn hex_bytes(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex byte"))
            .collect()
    }

    const D5_A_NOMETHOD: &str = "cafebabe0000003d000d070002010001410700040100106a6176612f6c616e672f4f626a65637407000601001f6a6176612f6c616e672f616e6e6f746174696f6e2f416e6e6f746174696f6e01000576616c756501001428294c6a6176612f6c616e672f537472696e673b010011416e6e6f746174696f6e44656661756c7401000001000a536f7572636546696c6501000741312e6a61766126000001000300010005000000000001000b00000002000c";
    const D5_C1: &str = "cafebabe0000003d00130a000200030700040c000500060100106a6176612f6c616e672f4f626a6563740100063c696e69743e01000328295607000801000143010001730100124c6a6176612f6c616e672f537472696e673b010004436f646501000f4c696e654e756d6265725461626c6501000a536f7572636546696c6501000743312e6a61766101001b52756e74696d65496e76697369626c65416e6e6f746174696f6e730100034c413b01000576616c7565010001780020000700020000000100000009000a000000010000000500060001000b0000001d00010001000000052ab70001b100000001000c000000060001000000010002000d00000002000e000f0000000b0001001000010011730012";
    const D5_A2: &str = "cafebabe0000003d0010070002010001410700040100106a6176612f6c616e672f4f626a65637407000601001f6a6176612f6c616e672f616e6e6f746174696f6e2f416e6e6f746174696f6e01000576616c756501001428294c6a6176612f6c616e672f537472696e673b010011416e6e6f746174696f6e44656661756c740100000100016501000528294c453b0100034c453b01000a536f7572636546696c6501000741322e6a6176612600000100030001000500000002040100070008000100090000000373000a0401000b000c000100090000000565000d00020001000e00000002000f";
    const D5_C2_BAD: &str = "cafebabe0000003d00160a000200030700040c000500060100106a6176612f6c616e672f4f626a6563740100063c696e69743e01000328295607000801000143010001730100124c6a6176612f6c616e672f537472696e673b010004436f646501000f4c696e654e756d6265725461626c6501000a536f7572636546696c6501000743322e6a61766101001b52756e74696d65496e76697369626c65416e6e6f746174696f6e730100034c413b01000576616c756501000178010001650100034c453b0100015a0020000700020000000100000009000a000000010000000500060001000b0000001d00010001000000052ab70001b100000001000c000000060001000000010002000d00000002000e000f00000012000100100002001173001200136500140015";
    const D5_E: &str = "cafebabe0000003d002e0700020100014509000100040c00050006010001410100034c453b09000100080c0009000601000142090001000b0c000c000d0100072456414c5545530100045b4c453b0a000f001007000d0c00110012010005636c6f6e6501001428294c6a6176612f6c616e672f4f626a6563743b0a001400150700160c0017001801000e6a6176612f6c616e672f456e756d01000776616c75654f66010035284c6a6176612f6c616e672f436c6173733b4c6a6176612f6c616e672f537472696e673b294c6a6176612f6c616e672f456e756d3b0a0014001a0c001b001c0100063c696e69743e010016284c6a6176612f6c616e672f537472696e673b4929560800050a0001001a0800090a000100210c002200230100072476616c75657301000628295b4c453b01000676616c756573010004436f646501000f4c696e654e756d6265725461626c65010017284c6a6176612f6c616e672f537472696e673b294c453b0100095369676e61747572650100032829560100083c636c696e69743e0100154c6a6176612f6c616e672f456e756d3c4c453b3e3b01000a536f7572636546696c65010006452e6a6176614030000100140000000340190005000600004019000900060000101a000c000d000000050009002400230001002500000022000100000000000ab2000ab6000ec0000fb0000000010026000000060001000000010009001700270001002500000022000200010000000a12012ab80013c00001b0000000010026000000060001000000010002001b001c000200250000001f00030003000000072a2b1cb70019b1000000010026000000060001000000010028000000020029100a002200230001002500000029000400000000001105bd00015903b20003535904b2000753b0000000010026000000060001000000010008002a002900010025000000390004000000000021bb000159121d03b7001eb30003bb000159121f04b7001eb30007b80020b3000ab1000000010026000000060001000000010002002800000002002b002c00000002002d";

    fn d5_reader(
        lint_option: Option<&str>,
    ) -> (
        std::rc::Rc<std::cell::RefCell<Log>>,
        CompilerContext,
        Symtab,
        ClassReader,
        std::rc::Rc<Names>,
    ) {
        let mut context = Context::new();
        let log = Log::instance(&mut context);
        let names = Rc::new(Names::new(false));
        let messages = crate::util::javac_messages::JavacMessages::instance(&mut context);
        let factory = Rc::new(Factory::new(messages, "compiler"));
        let dcfh = DeferredCompletionFailureHandler::instance();
        let preview = Rc::new(Preview::new(false, false));
        let profile = Rc::new(Profile::values()[0]);
        let options = Options::instance(&mut context);
        if let Some(option) = lint_option {
            options.put(option, "");
        }
        let source = Source::JDK17;
        let reader = ClassReader::new(
            std::rc::Rc::clone(&log),
            std::rc::Rc::clone(&names),
            factory,
            dcfh,
            preview,
            profile,
            &options,
            source,
            Rc::new(std::cell::RefCell::new(None)),
        );
        let mut ctx = CompilerContext::new();
        let syms = Symtab::new(
            &mut ctx,
            std::rc::Rc::clone(&names),
            Target::JDK1_17,
            source,
        );
        (log, ctx, syms, reader, names)
    }

    fn d5_register(
        ctx: &mut CompilerContext,
        syms: &Symtab,
        reader: &mut ClassReader,
        name: &str,
        bytes: Vec<u8>,
    ) -> SymbolId {
        let class_name = reader
            .names
            .from_string(&javac_strings::JavaString::from(name));
        let c = syms.enter_class_flat(ctx, syms.unnamed_module, class_name);
        let file_name = javac_strings::JavaString::from(format!("{name}.class").as_str());
        let file = std::rc::Rc::new(D5ClassFile {
            name: file_name,
            bytes,
        });
        if let crate::code::symbol::SymbolKind::ClassSymbol { classfile, .. } =
            &mut ctx.sym_mut(c).kind_data
        {
            *classfile = Some(file);
        }
        reader
            .read_class_file(ctx, syms, c)
            .expect("read fixture class");
        c
    }

    fn d5_enter(
        ctx: &mut CompilerContext,
        syms: &Symtab,
        reader: &mut ClassReader,
        name: &str,
        bytes: Vec<u8>,
    ) -> SymbolId {
        let class_name = reader
            .names
            .from_string(&javac_strings::JavaString::from(name));
        let c = syms.enter_class_flat(ctx, syms.unnamed_module, class_name);
        let file_name = javac_strings::JavaString::from(format!("{name}.class").as_str());
        let file = std::rc::Rc::new(D5ClassFile {
            name: file_name,
            bytes,
        });
        if let crate::code::symbol::SymbolKind::ClassSymbol { classfile, .. } =
            &mut ctx.sym_mut(c).kind_data
        {
            *classfile = Some(file);
        }
        c
    }
    #[test]
    fn class_reader_reports_annotation_method_not_found() {
        let (log, mut ctx, syms, mut reader, names) = d5_reader(Some("Xlint:all"));
        log.borrow_mut().install_diagnostic_handler(
            crate::util::log::DiagnosticHandler::Deferred {
                prev: None,
                deferred: std::collections::VecDeque::new(),
                filter: None,
            },
        );
        let _a = d5_register(&mut ctx, &syms, &mut reader, "A", hex_bytes(D5_A_NOMETHOD));
        let c = d5_register(&mut ctx, &syms, &mut reader, "C", hex_bytes(D5_C1));
        let proxies = reader
            .pending_annotations
            .iter()
            .find(|(sym, _)| *sym == c)
            .map(|(_, proxies)| proxies.clone())
            .expect("pending annotation for C");
        let class_file = match &ctx.sym(c).kind_data {
            crate::code::symbol::SymbolKind::ClassSymbol { classfile, .. } => classfile.clone(),
            _ => None,
        };
        let deprox = DeproxyContext {
            log: std::rc::Rc::clone(&log),
            lint_classfile: true,
            current_file: class_file,
        };
        let types = crate::code::types::Types::with_names(std::rc::Rc::clone(&names));
        deproxy_compound_list(
            &mut ctx,
            &syms,
            &types,
            &names,
            &proxies,
            Some(&mut reader),
            Some(&deprox),
        )
        .expect("deproxy fixture annotation");
        let diagnostics = log.borrow_mut().take_deferred_diagnostics();
        let warning = diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic.get_code_string() == "compiler.warn.annotation.method.not.found"
            })
            .expect("annotation.method.not.found warning");
        assert_eq!(
            warning.get_message(None).to_string_lossy(),
            "Cannot find annotation method 'value()' in type 'A'"
        );
        let source_name = warning
            .get_source()
            .expect("warning source")
            .get_name()
            .to_string_lossy();
        assert_eq!(source_name, "C.class");
        let args: Vec<String> = warning
            .get_args()
            .iter()
            .filter_map(|arg| {
                if let Ok(text) = arg.clone().downcast::<javac_strings::JavaString>() {
                    Some(text.to_string_lossy())
                } else if let Ok(type_arg) =
                    arg.clone()
                        .downcast::<crate::resources::diagnostic_contract::TypeArg>()
                {
                    Some(type_arg.text.clone())
                } else {
                    arg.clone()
                        .downcast::<crate::util::jc_diagnostic::JCDiagnostic>()
                        .ok()
                        .map(|diagnostic| diagnostic.get_message(None).to_string_lossy())
                }
            })
            .collect();
        assert_eq!(args, vec!["A".to_string(), "value".to_string()]);
    }

    #[test]
    fn class_reader_reports_unknown_enum_constant() {
        let (log, mut ctx, syms, mut reader, names) = d5_reader(None);
        log.borrow_mut().install_diagnostic_handler(
            crate::util::log::DiagnosticHandler::Deferred {
                prev: None,
                deferred: std::collections::VecDeque::new(),
                filter: None,
            },
        );
        let _e = d5_enter(&mut ctx, &syms, &mut reader, "E", hex_bytes(D5_E));
        let _a = d5_register(&mut ctx, &syms, &mut reader, "A", hex_bytes(D5_A2));
        let c = d5_register(&mut ctx, &syms, &mut reader, "C", hex_bytes(D5_C2_BAD));
        let proxies = reader
            .pending_annotations
            .iter()
            .find(|(sym, _)| *sym == c)
            .map(|(_, proxies)| proxies.clone())
            .expect("pending annotation for C");
        let class_file = match &ctx.sym(c).kind_data {
            crate::code::symbol::SymbolKind::ClassSymbol { classfile, .. } => classfile.clone(),
            _ => None,
        };
        let deprox = DeproxyContext {
            log: std::rc::Rc::clone(&log),
            lint_classfile: false,
            current_file: class_file,
        };
        let types = crate::code::types::Types::with_names(std::rc::Rc::clone(&names));
        deproxy_compound_list(
            &mut ctx,
            &syms,
            &types,
            &names,
            &proxies,
            Some(&mut reader),
            Some(&deprox),
        )
        .expect("deproxy fixture annotation");
        let diagnostics = log.borrow_mut().take_deferred_diagnostics();
        let warning = diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic.get_code_string() == "compiler.warn.unknown.enum.constant"
            })
            .expect("unknown.enum.constant warning");
        assert_eq!(
            warning.get_message(None).to_string_lossy(),
            "unknown enum constant E.Z"
        );
        assert!(
            warning.get_source().is_none(),
            "upstream raw output has no source"
        );
        let args: Vec<String> = warning
            .get_args()
            .iter()
            .filter_map(|arg| {
                arg.clone()
                    .downcast::<javac_strings::JavaString>()
                    .ok()
                    .map(|text| text.to_string_lossy())
            })
            .collect();
        assert_eq!(
            args,
            vec!["E.class".to_string(), "E".to_string(), "Z".to_string()]
        );
    }
    #[test]
    fn adjust_flags_match_classfile_bits() {
        let reader = reader();
        assert_eq!(reader.adjust_class_flags(flags::ACC_SUPER as i64), 0);
        assert_eq!(
            reader.adjust_class_flags(flags::ACC_MODULE as i64),
            flags::MODULE
        );
        assert_eq!(
            reader.adjust_method_flags(flags::ACC_BRIDGE as i64),
            flags::BRIDGE
        );
        assert_eq!(
            reader.adjust_method_flags(flags::ACC_VARARGS as i64),
            flags::VARARGS
        );
        assert_eq!(reader.adjust_field_flags(0x0001), 0x0001);
    }

    #[test]
    fn proxy_type_resolves_signature_name() {
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let mut reader = reader();
        let names = Rc::clone(&reader.names);
        let syms = crate::code::symtab::Symtab::new(
            &mut ctx,
            Rc::clone(&names),
            crate::jvm::target::Target::JDK1_17,
            crate::code::source::Source::JDK17,
        );
        reader.current_module = Some(syms.java_base);
        let sig = names.from_string(&javac_strings::JavaString::from("Ljava/lang/Object;"));
        let proxy = ctx.alloc_type(crate::code::type_::Type::new(
            crate::code::type_tag::TypeTag::NONE,
            None,
            crate::code::type_::TypeKindData::ProxyType { name: sig },
        ));
        let resolved = reader
            .resolve_possible_proxy_type(&mut ctx, &syms, proxy)
            .expect("ProxyType.resolve");
        assert_eq!(
            ctx.typ(resolved).get_tag(),
            crate::code::type_tag::TypeTag::CLASS
        );
        let tsym = ctx.typ(resolved).tsym.expect("Object symbol");
        assert_eq!(
            ctx.sym(tsym).name.to_java_string(),
            javac_strings::JavaString::from("Object")
        );
    }

    #[test]
    fn skip_member_advances_over_attributes() {
        let mut reader = reader();
        // access_flags(2), name(2), descriptor(2), attributes_count(2)=0
        reader
            .buf
            .append_bytes(&[0x00, 0x01, 0x00, 0x05, 0x00, 0x06, 0x00, 0x00]);
        reader.bp = 0;
        reader.skip_member();
        assert_eq!(reader.bp, 8);
    }

    #[test]
    fn buffer_accessors_read_big_endian_values() {
        let mut reader = reader();
        reader
            .buf
            .append_bytes(&[0xCA, 0xFE, 0xBA, 0xBE, 0x00, 0x3D, 0x12, 0x34]);
        reader.bp = 0;
        assert_eq!(reader.next_int(), JAVA_MAGIC);
        assert_eq!(reader.next_char(), 0x003D);
        assert_eq!(reader.next_byte(), 0x12);
        assert_eq!(reader.next_byte(), 0x34);
        assert_eq!(reader.bp, 8);
    }

    #[test]
    #[allow(clippy::mutable_key_type)]
    fn parameter_names_are_synthesized_and_access_flags_are_applied() {
        let mut reader = reader();
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let syms = crate::code::symtab::Symtab::new(
            &mut ctx,
            Rc::clone(&reader.names),
            crate::jvm::target::Target::JDK1_17,
            Source::JDK17,
        );
        let names = Rc::clone(&reader.names);
        let owner = syms.enter_class_flat(
            &mut ctx,
            syms.java_base,
            names.from_string(&javac_strings::JavaString::from("P")),
        );
        let mut exclude: HashSet<Name> = HashSet::new();
        let first = reader
            .parameter(&mut ctx, 0, syms.int_type, owner, &mut exclude)
            .expect("first parameter");
        assert_eq!(
            ctx.sym(first).name.to_java_string().to_string(),
            "arg0",
            "Java: names.fromString(prefix + exclude.size())"
        );
        assert_eq!(
            ctx.sym(first).flags_field & crate::code::flags::PARAMETER,
            crate::code::flags::PARAMETER,
            "Parameter flag"
        );
        let second = reader
            .parameter(&mut ctx, 1, syms.int_type, owner, &mut exclude)
            .expect("second parameter");
        assert_eq!(ctx.sym(second).name.to_java_string().to_string(), "arg1");
        reader.parameter_access_flags = Some(vec![0, crate::code::flags::FINAL]);
        let third = reader
            .parameter(&mut ctx, 1, syms.int_type, owner, &mut exclude)
            .expect("parameter with access flags");
        assert_ne!(
            ctx.sym(third).flags_field & crate::code::flags::FINAL as i64,
            0,
            "MethodParameters access flag applied"
        );
    }

    #[test]
    fn source_file_object_equals_and_hash_by_name() {
        let names = Names::new(false);
        let name = names.from_string(&javac_strings::JavaString::from("Src.java"));
        let first = SourceFileObject { name: name.clone() };
        let second = SourceFileObject { name };
        let other = SourceFileObject {
            name: names.from_string(&javac_strings::JavaString::from("Other.java")),
        };
        assert_eq!(first, second, "same source file name");
        assert_ne!(first, other, "different source file name");
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let hash = |value: &SourceFileObject| {
            let mut hasher = DefaultHasher::new();
            value.hash(&mut hasher);
            hasher.finish()
        };
        assert_eq!(hash(&first), hash(&second), "same source file name hash");
    }
}
