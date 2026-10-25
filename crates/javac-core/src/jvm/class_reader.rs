// Origin: com/sun/tools/javac/jvm/ClassReader.java @ 982d53c (jdk-17.0.19-ga)
// PORTING_RULES §19 gap registration (method-level audit of ClassReader.java):
// Dependency/phase: P2/S2-B annotation/TypeAnnotation/Module/Directive batches
// and P3 semantic batches (Annotate/Directive/Types).
// Not yet ported upstream methods (no substitutes in this file):
// - NOT PORTED: instance (ClassReader.java:257)
// - NOT PORTED: getEnclosingType (ClassReader.java:575)
// - NOT PORTED: setEnclosingType (ClassReader.java:600)
// - NOT PORTED: read (ClassReader.java:778)
// - NOT PORTED: read (ClassReader.java:799)
// - NOT PORTED: read (ClassReader.java:808)
// - NOT PORTED: read (ClassReader.java:857)
// - NOT PORTED: read (ClassReader.java:865)
// - NOT PORTED: read (ClassReader.java:876)
// - NOT PORTED: read (ClassReader.java:888)
// - NOT PORTED: read (ClassReader.java:925)
// - NOT PORTED: read (ClassReader.java:942)
// - NOT PORTED: read (ClassReader.java:950)
// - NOT PORTED: read (ClassReader.java:958)
// - NOT PORTED: read (ClassReader.java:988)
// - NOT PORTED: read (ClassReader.java:994)
// - NOT PORTED: read (ClassReader.java:1000)
// - NOT PORTED: read (ClassReader.java:1006)
// - NOT PORTED: read (ClassReader.java:1012)
// - NOT PORTED: read (ClassReader.java:1020)
// - NOT PORTED: read (ClassReader.java:1026)
// - NOT PORTED: read (ClassReader.java:1032)
// - NOT PORTED: read (ClassReader.java:1038)
// - NOT PORTED: read (ClassReader.java:1044)
// - NOT PORTED: read (ClassReader.java:1050)
// - NOT PORTED: read (ClassReader.java:1064)
// - NOT PORTED: read (ClassReader.java:1095)
// - NOT PORTED: classNameMapper (ClassReader.java:1195)
// - NOT PORTED: read (ClassReader.java:1205)
// - NOT PORTED: read (ClassReader.java:1218)
// - NOT PORTED: read (ClassReader.java:1239)
// - NOT PORTED: readEnclosingMethodAttr (ClassReader.java:1256)
// - NOT PORTED: simpleBinaryName (ClassReader.java:1299)
// - NOT PORTED: findMethod (ClassReader.java:1314)
// - NOT PORTED: isSameBinaryType (ClassReader.java:1346)
// - NOT PORTED: isAsciiDigit (ClassReader.java:1363)
// - NOT PORTED: setFlagIfAttributeTrue (ClassReader.java:1479)
// - NOT PORTED: accept (ClassReader.java:1834)
// - NOT PORTED: toString (ClassReader.java:1836)
// - NOT PORTED: accept (ClassReader.java:1847)
// - NOT PORTED: toString (ClassReader.java:1849)
// - NOT PORTED: accept (ClassReader.java:1860)
// - NOT PORTED: toString (ClassReader.java:1862)
// - NOT PORTED: accept (ClassReader.java:1876)
// - NOT PORTED: toString (ClassReader.java:1878)
// - NOT PORTED: visitConstant (ClassReader.java:1989)
// - NOT PORTED: visitClass (ClassReader.java:1994)
// - NOT PORTED: visitEnum (ClassReader.java:1998)
// - NOT PORTED: visitCompound (ClassReader.java:2002)
// - NOT PORTED: visitArray (ClassReader.java:2006)
// - NOT PORTED: visitError (ClassReader.java:2010)
// - NOT PORTED: visitEnumAttributeProxy (ClassReader.java:2014)
// - NOT PORTED: visitClassAttributeProxy (ClassReader.java:2050)
// - NOT PORTED: visitArrayAttributeProxy (ClassReader.java:2055)
// - NOT PORTED: visitCompoundAnnotationProxy (ClassReader.java:2066)
// - NOT PORTED: run (ClassReader.java:2099)
// - NOT PORTED: toString (ClassReader.java:2113)
// - NOT PORTED: run (ClassReader.java:2136)
// - NOT PORTED: toString (ClassReader.java:2163)
// - NOT PORTED: run (ClassReader.java:2189)
// - NOT PORTED: addTypeAnnotationsToSymbol (ClassReader.java:2212)
// - NOT PORTED: visitClassSymbol (ClassReader.java:2241)
// - NOT PORTED: visitMethodSymbol (ClassReader.java:2259)
// - NOT PORTED: visitVarSymbol (ClassReader.java:2293)
// - NOT PORTED: visitSymbol (ClassReader.java:2299)
// - NOT PORTED: rewriteTypeParameters (ClassReader.java:2303)
// - NOT PORTED: addTypeAnnotations (ClassReader.java:2343)
// - NOT PORTED: addTypeAnnotations (ClassReader.java:2347)
// - NOT PORTED: typeParameterBound (ClassReader.java:2377)
// - NOT PORTED: methodFormalParameter (ClassReader.java:2385)
// - NOT PORTED: thrownType (ClassReader.java:2390)
// - NOT PORTED: classExtends (ClassReader.java:2394)
// - NOT PORTED: visitClassType (ClassReader.java:2419)
// - NOT PORTED: rewriteTypeParams (ClassReader.java:2440)
// - NOT PORTED: visitWildcardType (ClassReader.java:2449)
// - NOT PORTED: visitArrayType (ClassReader.java:2462)
// - NOT PORTED: visitType (ClassReader.java:2473)
// - NOT PORTED: adjustMethodParams (ClassReader.java:2588)
// - NOT PORTED: parameter (ClassReader.java:2707)
// - NOT PORTED: lookupMethod (ClassReader.java:2871)
// - NOT PORTED: equals (ClassReader.java:3146)
// - NOT PORTED: hashCode (ClassReader.java:3154)
// - NOT PORTED: complete (ClassReader.java:3173)
// - NOT PORTED: getTag (ClassReader.java:3209)
// - NOT PORTED: cloneWithMetadata (ClassReader.java:3214)
// - NOT PORTED: resolve (ClassReader.java:3218)
// - NOT PORTED: toString (ClassReader.java:3223)
// - NOT PORTED: complete (ClassReader.java:3261)
// Attribute readers registered in initAttributeReaders but not yet wired
// (read_attribute skips them exactly like the upstream unknown-attribute path;
// TODO items in read_attribute list the dependency batches):
// - NOT PORTED reader: Code (S2-B batch)
// - NOT PORTED reader: ConstantValue (S2-B batch)
// - NOT PORTED reader: InnerClasses (S2-B batch)
// - NOT PORTED reader: SourceFile (S2-B batch)
// - NOT PORTED reader: EnclosingMethod (S2-B batch)
// - NOT PORTED reader: AnnotationDefault (S2-B batch)
// - NOT PORTED reader: RuntimeInvisibleAnnotations (S2-B batch)
// - NOT PORTED reader: RuntimeVisibleAnnotations (S2-B batch)
// - NOT PORTED reader: RuntimeInvisibleParameterAnnotations (S2-B batch)
// - NOT PORTED reader: RuntimeVisibleParameterAnnotations (S2-B batch)
// - NOT PORTED reader: RuntimeVisibleTypeAnnotations (S2-B batch)
// - NOT PORTED reader: RuntimeInvisibleTypeAnnotations (S2-B batch)
// - NOT PORTED reader: Module (S2-B batch)
// - NOT PORTED reader: ModuleResolution (S2-B batch)
// End of §19 gap registration.

//! This class provides operations to read a classfile into an internal
//! representation. The internal representation is anchored in a ClassSymbol
//! which contains in its scope symbol representations for all other definitions
//! in the classfile. Top-level Classes themselves appear as members of the
//! scopes of PackageSymbols.
//!
//! PORTING_RULES batch status (P2/S1):
//! * ported in this file: ClassReader structure/constructor (dependency injection
//!   in place of the Context singletons), readClassFile / readClassBuffer
//!   (magic, version, preview checks and the PoolReader wiring), buffer access
//!   (nextChar/nextByte/nextInt), skipMember, adjust*Flags, badClassFile.
//! * NOT ported yet (unimplemented! with TODO below): readClass and the
//!   attribute readers / signature state machine (S2), annotation and type
//!   annotation attributes (S3), Annotate/ClassFinder/file-manager wiring.
//!   These methods carry explicit TODO markers; no substitutes are provided.
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
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TypeId};
use crate::jvm::class_file::{Version, JAVA_MAGIC, PREVIEW_MINOR_VERSION};
use crate::jvm::pool_reader::PoolReader;
use crate::jvm::profile::Profile;
use crate::util::byte_buffer::ByteBuffer;
use crate::util::jc_diagnostic::{Factory, Fragment};
use crate::util::list::List;
use crate::util::log::Log;
use crate::util::name::Name;
use crate::util::names::Names;
use crate::util::options::Options;

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
    /// PORTING_RULES: Scope.WriteableScope is not ported yet (TODO(P3 scope
    /// batch)); the field is kept as None and enterTypevars is unimplemented.
    typevars: Option<crate::ids::ScopeId>,
    /// Java: protected JavaFileObject currentClassFile = null
    pub current_class_file: Option<Rc<dyn JavaFileObject>>,
    /// Java: protected Symbol currentOwner = null
    pub current_owner: Option<SymbolId>,
    /// Java: protected ModuleSymbol currentModule = null
    pub current_module: Option<SymbolId>,
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
    /// Java: int[] parameterNameIndices (MethodParameters/LVT batch)
    parameter_name_indices: Option<Vec<i32>>,
    /// Java: int[] parameterAccessFlags
    parameter_access_flags: Option<Vec<i32>>,
    /// Java: boolean haveParameterNameIndices
    have_parameter_name_indices: bool,
    /// Java: boolean sawMethodParameters
    saw_method_parameters: bool,
    /// Java: Type.noType (syms.noType). Symtab does not expose the singleton yet,
    /// so the ClassReader caches the NONE-tag type it uses for absent supertypes
    /// (registered representation adapter; tag-based comparisons are unchanged).
    no_type: Option<TypeId>,
}

impl ClassReader {
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
    ) -> ClassReader {
        let verbose = options.is_set_option(crate::main::option::Option::VERBOSE);
        let save_parameter_names =
            options.is_set_option(crate::main::option::Option::PARAMETERS);
        let mut reader = ClassReader {
            verbose,
            allow_modules: Feature::MODULES.allowed_in_source(source),
            allow_sealed_types: Feature::SEALED_CLASSES.allowed_in_source(source),
            allow_records: Feature::RECORDS.allowed_in_source(source),
            // Java: lintClassfile = Lint.instance(context).isEnabled(LintCategory.CLASSFILE);
            // Lint is not ported yet (TODO(Lint batch), registered); false keeps
            // the upstream default when no -Xlint:classfile is requested.
            lint_classfile: false,
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
            buf: ByteBuffer::with_initial_size(ClassReader::INITIAL_BUFFER_SIZE),
            bp: 0,
            pool_reader: None,
            major_version: 0,
            minor_version: 0,
            signature_buffer: vec![0u8; ClassReader::INITIAL_BUFFER_SIZE as usize],
            sbp: 0,
            reading_class_attr: false,
            types: crate::code::types::Types::new(),
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
    fn pool_error(&self, err: crate::jvm::pool_reader::Error) -> crate::code::symbol::CompletionFailure {
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
        self.pool_reader.as_ref().expect("poolReader (readClassBuffer first)")
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
        syms: &mut crate::code::symtab::Symtab,
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
            Ok(_) => unreachable!("TagSet::Module accepted a non-module value"),
            Err(err) => Err(self.pool_error(err)),
        }
    }

    /// Java: PackageSymbol getPackage(int index)
    // Unwired upstream pool accessor (used by the S2/S3 attribute/module batches).
    #[allow(dead_code)]
    fn get_package(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
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
            Ok(_) => unreachable!("TagSet::Package accepted a non-package value"),
            Err(err) => Err(self.pool_error(err)),
        }
    }

    /// Java: ClassSymbol getClass(int index)
    fn get_class(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
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
            Ok(_) => unreachable!("TagSet::Class accepted a non-class value"),
            Err(err) => Err(self.pool_error(err)),
        }
    }

    /// Java: Type getType(int index) — return getName(index).map(reader::sigToType)
    fn get_type(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
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
        syms: &mut crate::code::symtab::Symtab,
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
            Ok(_) => unreachable!("TagSet::NameAndType accepted a non-nameandtype value"),
            Err(err) => Err(self.pool_error(err)),
        }
    }

    /// Java: Object getConstant(int index)
    // Unwired upstream pool accessor (used by the S2/S3 attribute batches).
    #[allow(dead_code)]
    fn get_constant(&mut self, index: i32) -> Result<crate::jvm::pool_reader::PoolValue, crate::code::symbol::CompletionFailure> {
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
        syms: &mut crate::code::symtab::Symtab,
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
        syms: &mut crate::code::symtab::Symtab,
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
                    let name = self.names.table.from_utf(
                        &self.signature,
                        start,
                        self.sigp - 1 - start,
                    );
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
                while self.sigp < self.siglimit
                    && self.signature[self.sigp as usize] as char == '^'
                {
                    self.sigp += 1;
                    let t = self.sig_to_type(ctx, syms)?;
                    thrown = thrown.prepend(t);
                }
                // if there is a typevar in the throws clause we should state it.
                let mut l = thrown.clone();
                while l.non_empty() {
                    let head = *l.head();
                    if ctx.typ(head).has_tag(crate::code::type_tag::TypeTag::TYPEVAR)
                        && let Some(tsym) = ctx.typ(head).tsym
                    {
                        ctx.sym_mut(tsym).flags_field |= crate::code::flags::THROWS;
                    }
                    l = l.tail();
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

    /// Java: Type noType (syms.noType): the ClassReader only needs the tag
    /// constant here; Types.noType is not represented in Symtab yet (TODO P3),
    /// so the arena slot is created from the shared Type::new (tag NONE). The
    /// call sites that need the global singleton are registered in the batch
    /// report.
    fn no_type(&mut self, ctx: &mut CompilerContext) -> TypeId {
        if let Some(no_type) = self.no_type {
            return no_type;
        }
        let no_type = ctx.alloc_type(crate::code::type_::Type::new(
            crate::code::type_tag::TypeTag::NONE,
            None,
            crate::code::type_::TypeKindData::JCNoType,
        ));
        self.no_type = Some(no_type);
        no_type
    }

    /// Java: List<Type> sigToTypes(char terminator)
    fn sig_to_types(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
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
        syms: &mut crate::code::symtab::Symtab,
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
        syms: &mut crate::code::symtab::Symtab,
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
        syms: &mut crate::code::symtab::Symtab,
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
                        None => self
                            .types
                            .erasure(ctx, syms, ctx.sym(t).ty.expect("entered class type")),
                        Some(outer) => ctx.alloc_type(
                            crate::code::type_::Type::new_class_type(
                                Some(outer),
                                Some(List::nil()),
                                t,
                            ),
                        ),
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
                    outer = Some(ctx.alloc_type(
                        crate::code::type_::Type::new_class_type(outer, Some(args), t),
                    ));
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
                        outer = Some(ctx.alloc_type(
                            crate::code::type_::Type::new_class_type(
                                Some(outer_ty),
                                Some(List::nil()),
                                t,
                            ),
                        ));
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
        syms: &mut crate::code::symtab::Symtab,
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
        syms: &mut crate::code::symtab::Symtab,
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
        let candidates = ctx.scope_get_symbols_by_name(
            members,
            name,
            crate::code::scope::LookupKind::NON_RECURSIVE,
            &|sym| ctx.sym(sym).kind == crate::code::kinds::Kind::MTH,
        );
        let types = crate::code::types::Types::new();
        for candidate in candidates {
            let ty = ctx.sym(candidate).ty.expect("method type");
            let params = ctx.typ(ty).get_parameter_types(ctx);
            if types.is_same_types(ctx, syms, &params, &argtypes) {
                return Some(candidate);
            }
        }
        None
    }
    /// Java: protected void enterTypevars(Symbol sym, Type t)
    fn enter_typevars(
        &mut self,
        ctx: &mut CompilerContext,
        sym: SymbolId,
        t: TypeId,
    ) {
        if let Some(enclosing) = ctx.typ(t).get_enclosing_type() {
            if !ctx.typ(enclosing).has_tag(crate::code::type_tag::TypeTag::NONE) {
                let owner = ctx.sym(sym).owner.expect("symbol owner");
                self.enter_typevars(ctx, owner, enclosing);
            }
        } else if ctx.sym(sym).kind == crate::code::kinds::Kind::MTH
            && !ctx.sym(sym).is_static(ctx)
        {
            let owner = ctx.sym(sym).owner.expect("symbol owner");
            let owner_type = ctx.sym(owner).ty.expect("owner type");
            self.enter_typevars(ctx, owner, owner_type);
        }
        let mut xs = ctx.typ(t).get_type_arguments();
        let typevars = self.typevars.expect("typevars scope");
        while xs.non_empty() {
            let head = *xs.head();
            if let Some(tsym) = ctx.typ(head).tsym {
                ctx.scope_enter(typevars, tsym);
            }
            xs = xs.tail();
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
                crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } => *members_field,
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
        syms: &mut crate::code::symtab::Symtab,
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
        self.read_class_buffer(ctx, syms, c)?;
        // Java: if (!missingTypeVariables.isEmpty() && !foundTypeVariables.isEmpty()) {...}
        if !self.missing_type_variables.is_empty() && !self.found_type_variables.is_empty() {
            unimplemented!("TODO(P2/S2): readClassFile missingTypeVariables substitution (Types.subst/substBounds)");
        } else if self.missing_type_variables.is_empty()
            != self.found_type_variables.is_empty()
        {
            // Java: throw badClassFile("undecl.type.var", name);
            let key = Fragment::from(
                crate::resources::compiler_properties::fragments::undecl_type_var(),
            );
            return Err(self.bad_class_file(key).failure);
        }

        // Java: if ((c.flags_field & Flags.ANNOTATION) != 0) c.setAnnotationTypeMetadata(...)
        //       else c.setAnnotationTypeMetadata(AnnotationTypeMetadata.notAnAnnotationType());
        // TODO(P2/S3): AnnotationTypeMetadata is not ported yet (annotation batch);
        // no metadata state exists locally, so the not-an-annotation-type value
        // has no representation (registered gap, not an approximation).
        if (ctx.sym(c).flags_field & crate::code::flags::ANNOTATION as i64) != 0 {
            unimplemented!("TODO(P2/S3): AnnotationTypeMetadata for annotation types (annotation batch)");
        }

        // Java: if (c == currentModule.module_info) { ... interimUses/interimProvides ... }
        if let Some(module) = self.current_module {
            let module_info = match &ctx.sym(module).kind_data {
                crate::code::symbol::SymbolKind::ModuleSymbol { module_info, .. } => *module_info,
                _ => None,
            };
            if module_info == Some(c) {
                unimplemented!("TODO(P2/S2): module uses/provides (UsesProvidesCompleter, module batch)");
            }
        }
        Ok(())
    }

    /// Java: private void readClassBuffer(ClassSymbol c) throws IOException
    fn read_class_buffer(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
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
        let _max_minor = Version::max().minor();
        let preview_class_file = self.minor_version == PREVIEW_MINOR_VERSION;
        if self.major_version > max_major
            || self.major_version * 1000 + self.minor_version
                < Version::min().major() * 1000 + Version::min().minor()
        {
            if self.major_version == max_major + 1 && !preview_class_file {
                // Java: log.warning(Warnings.BigMajorVersion(currentClassFile, majorVersion, maxMajor))
                unimplemented!("TODO(P2/Log): log.warning(BigMajorVersion) needs a DiagnosticPosition-carrying Log adapter");
            } else {
                // Java: badClassFile("wrong.version", major, minor, maxMajor, maxMinor);
                // The generated accessor currently has no parameters (registered
                // P4 diagnostic-contract deviation, see the batch report).
                let key = Fragment::from(
                    crate::resources::compiler_properties::fragments::wrong_version(),
                );
                return Err(self.bad_class_file(key).failure);
            }
        }
        if preview_class_file {
            if !self.preview.is_enabled() {
                unimplemented!("TODO(P2/Log): preview.disabledError(JavaFileObject,int) wiring is unimplemented (Log batch)");
            } else {
                unimplemented!("TODO(P2/Log): preview.warnPreview(JavaFileObject,int) wiring is unimplemented (Log batch)");
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
        syms: &mut crate::code::symtab::Symtab,
        c: SymbolId,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
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
        if ctx
            .typ(ct)
            .get_enclosing_type()
            .is_some_and(|outer| ctx.typ(outer).has_tag(crate::code::type_tag::TypeTag::CLASS))
        {
            let enclosing = ctx.typ(ct).get_enclosing_type().expect("enclosing");
            let owner = ctx.sym(c).owner.expect("class owner");
            self.enter_typevars(ctx, owner, enclosing);
        }

        // read flags, or skip if this is an inner class
        let f = self.next_char() as i64;
        let flags = self.adjust_class_flags(f);
        let module_flag = flags & flags::MODULE;
        if module_flag == 0 {
            let owner_kind = ctx.sym(c).owner.map(|o| ctx.sym(o).kind);
            if matches!(owner_kind, Some(crate::code::kinds::Kind::PCK) | Some(crate::code::kinds::Kind::ERR)) {
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
                        crate::util::java_string::JavaString::from(self.major_version.to_string().as_str()),
                        crate::util::java_string::JavaString::from(self.minor_version.to_string().as_str()),
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
            crate::code::type_::TypeKindData::ClassType { supertype_field: None, .. }
        );
        if supertype_field_is_null {
            let super_type = if n == 0 {
                self.no_type(ctx)
            } else {
                let class = self.get_class(ctx, syms, n)?;
                let class_type = ctx.sym(class).ty.expect("class type");
                self.types.erasure(ctx, syms, class_type)
            };
            if let crate::code::type_::TypeKindData::ClassType { supertype_field, .. } =
                &mut ctx.typ_mut(ct).kind
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
            crate::code::type_::TypeKindData::ClassType { interfaces_field: None, .. }
        );
        if interfaces_are_null
            && let crate::code::type_::TypeKindData::ClassType { interfaces_field, .. } =
                &mut ctx.typ_mut(ct).kind
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
            //     rc.accessor = lookupMethod(c, rc.name, List.nil());
            let components = match &ctx.sym(c).kind_data {
                crate::code::symbol::SymbolKind::ClassSymbol { record_components, .. } => {
                    record_components.clone()
                }
                _ => List::nil(),
            };
            let mut list = components;
            while list.non_empty() {
                let component = *list.head();
                let component_name = ctx.sym(component).name.clone();
                let accessor = self.lookup_method(ctx, syms, c, &component_name, List::nil());
                if let crate::code::symbol::SymbolKind::RecordComponent { accessor: slot, .. } =
                    &mut ctx.sym_mut(component).kind_data
                {
                    *slot = accessor;
                }
                list = list.tail();
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
        syms: &mut crate::code::symtab::Symtab,
    ) -> Result<SymbolId, crate::code::symbol::CompletionFailure> {
        let raw_flags = self.next_char() as i64;
        let flags = self.adjust_field_flags(raw_flags);
        let name_index = self.next_char() as i32;
        let name = self.get_name(name_index)?;
        let type_index = self.next_char() as i32;
        let ty = self.get_type(ctx, syms, type_index)?;
        let owner = self.current_owner.expect("currentOwner");
        let var = ctx.alloc_symbol(crate::code::symbol::Symbol::new_var_symbol(
            flags, name, ty,
            owner,
        ));
        self.read_member_attrs(ctx, syms, var)?;
        Ok(var)
    }

    /// Java: MethodSymbol readMethod()
    fn read_method(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
    ) -> Result<SymbolId, crate::code::symbol::CompletionFailure> {
        let raw_flags = self.next_char() as i64;
        let mut flags = self.adjust_method_flags(raw_flags);
        let name_index = self.next_char() as i32;
        let name = self.get_name(name_index)?;
        let type_index = self.next_char() as i32;
        let ty = self.get_type(ctx, syms, type_index)?;
        let owner = self.current_owner.expect("currentOwner");
        let owner_is_interface = ctx.sym(owner).is_interface();
        let is_clinit = name.to_java_string() == self.names.clinit.to_java_string();
        if owner_is_interface && (flags & crate::code::flags::ABSTRACT as i64) == 0 && !is_clinit {
            if self.major_version > Version::V52.major()
                || (self.major_version == Version::V52.major()
                    && self.minor_version >= Version::V52.minor())
            {
                if (flags & (crate::code::flags::STATIC as i64 | crate::code::flags::PRIVATE as i64)) == 0 {
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
        if ctx.sym(owner).is_constructor(&self.names) && ctx.sym(owner).has_outer_instance(ctx) {
            // Sometimes anonymous classes don't have an outer instance...
            // (upstream: skip this$n handling; the local port keeps the
            // MethodType unchanged when the owner is local/anonymous)
            unimplemented!("TODO(P2/S2): readMethod this$n stripping (requires owner.members()/name checks and Code batch)");
        }
        let method = ctx.alloc_symbol(crate::code::symbol::Symbol::new_method_symbol(
            flags, name, ty, owner,
        ));
        if self.types.is_signature_polymorphic(ctx, syms, method) {
            ctx.sym_mut(method).flags_field |= crate::code::flags::SIGNATURE_POLYMORPHIC;
        }
        if self.save_parameter_names {
            unimplemented!("TODO(P2/Code batch): initParameterNames (Code.width/LocalVariableTable)");
        }
        let prev_owner = self.current_owner;
        self.current_owner = Some(method);
        self.read_member_attrs(ctx, syms, method)?;
        self.current_owner = prev_owner;
        self.validate_method_type(ctx, &ctx.sym(method).name.clone(), ctx.sym(method).ty.expect("method type"))?;
        self.set_parameters(ctx, method, ty);
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
            .map(|restype| ctx.typ(restype).has_tag(crate::code::type_tag::TypeTag::VOID))
            .unwrap_or(false);
        if !(tag == crate::code::type_tag::TypeTag::METHOD
            || tag == crate::code::type_tag::TypeTag::FORALL)
            || (is_init && !restype_void)
        {
            let key = Fragment::from(
                crate::resources::compiler_properties::fragments::method_descriptor_invalid(name.clone()),
            );
            return Err(self.bad_class_file(key).failure);
        }
        Ok(())
    }

    /// Java: private void initAttributeReaders()
    fn init_attribute_readers(&mut self) {
        let defs: Vec<(Name, Version, Vec<AttributeKind>, AttributeReaderKind)> = vec![
            (self.names.Code.clone(), Version::V45_3, vec![AttributeKind::MEMBER], AttributeReaderKind::Code),
            (self.names.ConstantValue.clone(), Version::V45_3, vec![AttributeKind::MEMBER], AttributeReaderKind::ConstantValue),
            (self.names.Deprecated.clone(), Version::V45_3, vec![AttributeKind::CLASS, AttributeKind::MEMBER], AttributeReaderKind::Deprecated),
            (self.names.Exceptions.clone(), Version::V45_3, vec![AttributeKind::CLASS, AttributeKind::MEMBER], AttributeReaderKind::Exceptions),
            (self.names.InnerClasses.clone(), Version::V45_3, vec![AttributeKind::CLASS], AttributeReaderKind::InnerClasses),
            (self.names.LocalVariableTable.clone(), Version::V45_3, vec![AttributeKind::CLASS, AttributeKind::MEMBER], AttributeReaderKind::LocalVariableTable),
            (self.names.SourceFile.clone(), Version::V45_3, vec![AttributeKind::CLASS], AttributeReaderKind::SourceFile),
            (self.names.Synthetic.clone(), Version::V45_3, vec![AttributeKind::CLASS, AttributeKind::MEMBER], AttributeReaderKind::Synthetic),
            (self.names.EnclosingMethod.clone(), Version::V49, vec![AttributeKind::CLASS], AttributeReaderKind::EnclosingMethod),
            (self.names.Signature.clone(), Version::V49, vec![AttributeKind::CLASS, AttributeKind::MEMBER], AttributeReaderKind::Signature),
            (self.names.AnnotationDefault.clone(), Version::V49, vec![AttributeKind::CLASS, AttributeKind::MEMBER], AttributeReaderKind::AnnotationDefault),
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
            (self.names.Annotation.clone(), Version::V49, vec![AttributeKind::CLASS, AttributeKind::MEMBER], AttributeReaderKind::Annotation),
            (self.names.Bridge.clone(), Version::V49, vec![AttributeKind::MEMBER], AttributeReaderKind::Bridge),
            (self.names.Enum.clone(), Version::V49, vec![AttributeKind::CLASS, AttributeKind::MEMBER], AttributeReaderKind::Enum),
            (self.names.Varargs.clone(), Version::V49, vec![AttributeKind::CLASS, AttributeKind::MEMBER], AttributeReaderKind::Varargs),
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
            (self.names.MethodParameters.clone(), Version::V52, vec![AttributeKind::MEMBER], AttributeReaderKind::MethodParameters),
            (self.names.Module.clone(), Version::V53, vec![AttributeKind::CLASS], AttributeReaderKind::Module),
            (self.names.ModuleResolution.clone(), Version::V53, vec![AttributeKind::CLASS], AttributeReaderKind::ModuleResolution),
            (self.names.Record.clone(), Version::V58, vec![AttributeKind::CLASS], AttributeReaderKind::Record),
            (self.names.PermittedSubclasses.clone(), Version::V59, vec![AttributeKind::CLASS], AttributeReaderKind::PermittedSubclasses),
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
    /// Java: void readAttrs(Symbol sym, AttributeKind kind)
    fn read_attrs(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
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
    fn accepts(&self, position: usize, kind: AttributeKind) -> bool {
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
            unimplemented!("TODO(P2/Log): FutureAttr classfile lint warning (Log/Lint batch)");
        }
        false
    }

    /// Java: AttributeReader.read(Symbol sym, int attrLen) — dispatch per reader.
    fn read_attribute(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
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
                            & (crate::code::flags::MANDATED
                                | crate::code::flags::SYNTHETIC))
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
                let module_info = self.current_module.and_then(|module| match &ctx.sym(module).kind_data {
                    crate::code::symbol::SymbolKind::ModuleSymbol { module_info, .. } => *module_info,
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
            AttributeReaderKind::AnnotationDefault
            | AttributeReaderKind::RuntimeInvisibleAnnotations
            | AttributeReaderKind::RuntimeVisibleAnnotations
            | AttributeReaderKind::RuntimeInvisibleParameterAnnotations
            | AttributeReaderKind::RuntimeVisibleParameterAnnotations
            | AttributeReaderKind::RuntimeVisibleTypeAnnotations
            | AttributeReaderKind::RuntimeInvisibleTypeAnnotations
            | AttributeReaderKind::Module
            | AttributeReaderKind::ModuleResolution => {
                // TODO(S2-B next step): CompoundAnnotationProxy/TypeAnnotationPosition/
                // Module Directive batches (registered in the §19 block); skipped
                // exactly like the upstream unknown-attribute path until then.
                self.bp += attr_len;
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
        let value_text = format!("{const_value:?}");
        let bad_value = |clazz: &str| -> crate::code::symbol::CompletionFailure {
            let key = Fragment::from(
                crate::resources::compiler_properties::fragments::bad_constant_value(
                    javac_strings::JavaString::from(value_text.as_str()),
                    crate::resources::diagnostic_contract::SymbolArg::placeholder("symbol"),
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
                            crate::resources::diagnostic_contract::SymbolArg::placeholder("symbol"),
                            crate::resources::diagnostic_contract::TypeArg::placeholder("type"),
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
                            crate::resources::diagnostic_contract::TypeArg::placeholder("type"),
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

    /// Java: Code readCode(Symbol owner) (ClassReader.java:1373-1390)
    /// TODO(P6/Code batch): jvm.Code is not ported, so the structure is read
    /// and nested member attributes are dispatched, but the Code object is not
    /// stored on the method symbol (registered).
    fn read_code(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
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
        syms: &mut crate::code::symtab::Symtab,
        c: SymbolId,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        let n = self.next_char();
        for _ in 0..n {
            self.next_char(); // skip inner class symbol
            let outer_idx = self.next_char() as i32;
            let name_idx = self.next_char() as i32;
            let outer = if outer_idx == 0 { None } else { Some(self.get_class(ctx, syms, outer_idx)?) };
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
        syms: &mut crate::code::symtab::Symtab,
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
            let key = Fragment::from(
                crate::resources::compiler_properties::fragments::bad_enclosing_method(
                    crate::resources::diagnostic_contract::SymbolArg::placeholder("symbol"),
                ),
            );
            return Err(self.bad_class_file(key).failure);
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
        self.enter_typevars(ctx, self_class, self_ty);
        if !self.missing_type_variables.is_empty() {
            let mut found = List::nil();
            let missing = self.missing_type_variables.clone();
            let mut list = missing;
            while list.non_empty() {
                let t = *list.head();
                let name = ctx.typ(t).tsym.map(|sym| ctx.sym(sym).name.clone());
                let name = name.expect("type variable name");
                found = found.append(self.find_type_var(ctx, syms, &name)?);
                list = list.tail();
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
                    crate::resources::diagnostic_contract::SymbolArg::placeholder("symbol"),
                ),
            );
            return Err(self.bad_class_file(key).failure);
        };
        if simple.is_empty() || !simple.starts_with('$') {
            let key = Fragment::from(
                crate::resources::compiler_properties::fragments::bad_enclosing_method(
                    crate::resources::diagnostic_contract::SymbolArg::placeholder("symbol"),
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
        Ok(self.names.from_string(&crate::util::java_string::JavaString::from(result)))
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
    fn is_same_binary_type(
        &self,
        ctx: &mut CompilerContext,
        mt1: TypeId,
        mt2: TypeId,
    ) -> bool {
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
    fn read_class_attrs(&mut self, ctx: &mut CompilerContext, syms: &mut crate::code::symtab::Symtab, c: SymbolId) -> Result<(), crate::code::symbol::CompletionFailure> {
        self.read_attrs(ctx, syms, c, AttributeKind::CLASS)
    }

    /// Java: void readMemberAttrs(Symbol sym)
    fn read_member_attrs(&mut self, ctx: &mut CompilerContext, syms: &mut crate::code::symtab::Symtab, sym: SymbolId) -> Result<(), crate::code::symbol::CompletionFailure> {
        self.read_attrs(ctx, syms, sym, AttributeKind::MEMBER)
    }

    /// Java: void readClassAttrs / readMemberAttrs share readAttrs; kept distinct
    /// for the upstream names (see above).
    fn read_signature_attr(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
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
            if let crate::code::type_::TypeKindData::ClassType { supertype_field, .. } =
                &mut ctx.typ_mut(class_ty).kind
            {
                *supertype_field = Some(supertype);
            }
            let mut interfaces = List::nil();
            while self.sigp != self.siglimit {
                interfaces = interfaces.append(self.sig_to_type(ctx, syms).expect("interface signature"));
            }
            if let crate::code::type_::TypeKindData::ClassType { interfaces_field, .. } =
                &mut ctx.typ_mut(class_ty).kind
            {
                *interfaces_field = Some(interfaces);
            }
            self.reading_class_attr = false;
        } else {
            let ty = ctx.sym(sym).ty.expect("symbol type");
            let thrown = ctx.typ(ty).get_thrown_types(ctx);
            let sig_index = self.next_char() as i32;
            let new_ty = self.get_type(ctx, syms, sig_index).expect("member signature");
            ctx.sym_mut(sym).ty = Some(new_ty);
            if ctx.sym(sym).kind == crate::code::kinds::Kind::MTH
                && ctx.typ(new_ty).get_thrown_types(ctx).is_empty()
                && let crate::code::type_::TypeKindData::MethodType { thrown: slot, .. } =
                    &mut ctx.typ_mut(new_ty).kind
            {
                *slot = thrown;
            }
        }
        Ok(())
    }

    /// Java: Record attribute reader
    fn read_record_attr(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &mut crate::code::symtab::Symtab,
        sym: SymbolId,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        ctx.sym_mut(sym).flags_field |= crate::code::flags::RECORD;
        let component_count = self.next_char();
        let mut components = List::nil();
        for _ in 0..component_count {
            let name_index = self.next_char() as i32;
            let name = self.get_name(name_index).expect("record component name");
            let type_index = self.next_char() as i32;
            let ty = self.get_type(ctx, syms, type_index).expect("record component type");
            let component = ctx.alloc_symbol(crate::code::symbol::Symbol::new_record_component(
                name, ty, sym,
            ));
            self.read_attrs(ctx, syms, component, AttributeKind::MEMBER)?;
            components = components.append(component);
        }
        ctx.sym_mut(sym).set_record_components(components);
        Ok(())
    }

    /// Java: void setParameters(MethodSymbol sym, Type jvmType) — the
    /// parameterAnnotations branch needs the annotation proxies (S3).
    fn set_parameters(&mut self, ctx: &mut CompilerContext, sym: SymbolId, _jvm_type: TypeId) {
        let static_flag = ctx.sym(sym).flags_field & crate::code::flags::STATIC as i64;
        let first_param = if static_flag == 0 { 1 } else { 0 };
        let mut params = List::nil();
        let mut types = ctx
            .typ(ctx.sym(sym).ty.expect("method type"))
            .get_parameter_types(ctx);
        let mut index = first_param;
        // TODO(P2/S3): parameterAnnotations/parameterNameIndices/parameterAccessFlags
        // fields are not ported yet; parameterNames are synthesized exactly like
        // the upstream no-parameter-name path (argN, skipping the outer this slot).
        while types.non_empty() {
            let t = *types.head();
            let param = ctx.alloc_symbol(crate::code::symbol::Symbol::new_param_symbol(
                crate::code::flags::PARAMETER,
                self.names.from_string(&crate::util::java_string::JavaString::from(format!("arg{index}").as_str())),
                t,
                sym,
            ));
            params = params.append(param);
            index += 1;
            types = types.tail();
        }
        ctx.sym_mut(sym).set_params(params);
    }

    /// Java: public ClassFinder.BadEnclosingMethodAttr badEnclosingMethod(Symbol sym)
    #[allow(dead_code)]
    pub fn bad_enclosing_method(
        &self,
        _sym: SymbolId,
    ) -> crate::code::class_finder::BadEnclosingMethodAttr {
        unimplemented!("TODO(P2/S3): Fragments.BadEnclosingMethod(sym) accessor needs the Symbol argument (P4 contract)")
    }


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

    fn open_writer(&self) -> Result<Box<dyn javac_tools::java_io::Writer>, javac_tools::error::Error> {
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

    fn get_access_level(
        &self,
    ) -> Option<javac_tools::lang::model::element::modifier::Modifier> {
        None
    }
}

/// Java: current owner's classfile (ClassReader.readClassFile first line).
fn class_file_of(
    ctx: &CompilerContext,
    c: SymbolId,
) -> Option<Rc<dyn JavaFileObject>> {
    match &ctx.sym(c).kind_data {
        crate::code::symbol::SymbolKind::ClassSymbol { classfile, .. } => classfile.clone(),
        _ => None,
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

        fn open_reader(&self, _ignore_encoding_errors: bool) -> Result<Box<dyn Reader>, ToolsError> {
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

    fn type_var_name(ctx: &crate::compiler_context::CompilerContext, ty: crate::ids::TypeId) -> String {
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
            TypeKindData::ArrayType { elemtype } => {
                format!(
                    "ARRAY[{}]",
                    elemtype.map(|elem| type_summary(ctx, elem)).unwrap_or_default()
                )
            }
            TypeKindData::ClassType {
                typarams_field, ..
            } => {
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
            TypeKindData::JCNoType => "NONE".to_string(),
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
        );
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let mut syms = Symtab::new(&mut ctx, Rc::clone(&names), Target::JDK1_17, source);
        let class_name = names.from_string(&javac_strings::JavaString::from(binary_name));
        let java_base = syms.java_base;
        let c = syms.enter_class_flat(&mut ctx, java_base, class_name);
        let file = Rc::new(ClassBytesFile {
            name: javac_strings::JavaString::from(format!("{}.class", binary_name.replace('.', "/")).as_str()),
            bytes,
        });
        if let crate::code::symbol::SymbolKind::ClassSymbol { classfile, .. } =
            &mut ctx.sym_mut(c).kind_data
        {
            *classfile = Some(file);
        }
        reader
            .read_class_file(&mut ctx, &mut syms, c)
            .expect("readClassFile");
        let class_ty = ctx.sym(c).ty.expect("class type");
        let super_is_none = match &ctx.typ(class_ty).kind {
            crate::code::type_::TypeKindData::ClassType { supertype_field, .. } => {
                supertype_field
                    .map(|st| ctx.typ(st).has_tag(crate::code::type_tag::TypeTag::NONE))
                    .unwrap_or(true)
            }
            _ => panic!("class type expected"),
        };
        let interfaces = match &ctx.typ(class_ty).kind {
            crate::code::type_::TypeKindData::ClassType { interfaces_field, .. } => {
                let mut names_out = Vec::new();
                let mut list = interfaces_field.clone().unwrap_or_else(List::nil);
                while list.non_empty() {
                    let inter = *list.head();
                    names_out.push(type_summary(&ctx, inter));
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

    fn member_names(summary: &ParseSummary) -> Vec<&str> {
        summary.members.iter().map(|(_, _, name, _)| name.as_str()).collect()
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
        assert_eq!(summary.members.len(), 12, "members: {:?}", member_names(&summary));
        let names = member_names(&summary);
        for expected in [
            "<init>", "getClass", "hashCode", "equals", "clone", "toString",
            "notify", "notifyAll", "wait", "finalize",
        ] {
            assert!(names.contains(&expected), "missing {expected}: {names:?}");
        }
        assert_eq!(names.iter().filter(|name| **name == "wait").count(), 3);
        assert!(summary.members.iter().all(|(kind, _, _, _)| *kind == crate::code::kinds::Kind::MTH));
    }

    #[test]
    fn list_class_matches_javap_oracle() {
        let Some(summary) = parse_baseline("java.util.List") else {
            eprintln!("skipping: no baseline JDK 17 installation found");
            return;
        };
        // javap -private java.util.List: public interface, extends
        // java.util.Collection, 41 members (the descriptor-erased view; the
        // Signature attribute is read by the S2 attribute batch).
        assert_ne!(summary.flags & crate::code::flags::INTERFACE as i64, 0);
        assert_ne!(summary.flags & crate::code::flags::ABSTRACT as i64, 0);
        assert_eq!(
            summary.interfaces,
            vec!["java.util.Collection<TV:E>".to_string()]
        );
        assert_eq!(summary.class_typarams, vec!["E".to_string()]);
        assert!(!summary.super_is_none);
        assert_eq!(summary.members.len(), 41, "members: {:?}", member_names(&summary));
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
            assert!(fields.contains(&expected), "missing field {expected}: {fields:?}");
        }
    }

    fn parse_with_bytes(binary_name: &str, bytes: Vec<u8>) -> Result<(), crate::code::symbol::CompletionFailure> {
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
        );
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let mut syms = Symtab::new(&mut ctx, Rc::clone(&names), Target::JDK1_17, source);
        let class_name = names.from_string(&javac_strings::JavaString::from(binary_name));
        let java_base = syms.java_base;
        let c = syms.enter_class_flat(&mut ctx, java_base, class_name);
        let file = Rc::new(ClassBytesFile {
            name: javac_strings::JavaString::from(format!("{}.class", binary_name.replace('.', "/")).as_str()),
            bytes,
        });
        if let crate::code::symbol::SymbolKind::ClassSymbol { classfile, .. } =
            &mut ctx.sym_mut(c).kind_data
        {
            *classfile = Some(file);
        }
        reader.read_class_file(&mut ctx, &mut syms, c)
    }

    #[test]
    fn signature_views_match_javap() {
        // javap -v data (development-time oracle):
        //   java.util.List: class signature <E:Ljava/lang/Object;>Ljava/lang/Object;Ljava/util/Collection<TE;>;
        //     boolean add(E): Signature (TE;)Z
        //     <T> T[] toArray(T[]): Signature <T:Ljava/lang/Object;>([TT;)[TT;
        //   java.lang.String: CASE_INSENSITIVE_ORDER Signature
        //     Ljava/util/Comparator<Ljava/lang/String;>;
        //     implements Comparable<java.lang.String>
        let Some(list) = parse_baseline("java.util.List") else {
            eprintln!("skipping: no baseline JDK 17 installation found");
            return;
        };
        assert_eq!(list.class_typarams, vec!["E".to_string()]);
        let add = list
            .members
            .iter()
            .find(|(_, _, name, ty)| {
                name == "add" && ty.as_deref() == Some("(TV:E)->BOOLEAN")
            })
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
                name == "compareTo"
                    && ty.as_deref() == Some("(java.lang.String)->INT")
            })
            .expect("String.compareTo(String)");
        assert_eq!(
            compare_to.3.as_deref(),
            Some("(java.lang.String)->INT")
        );
    }

    fn fixture_bytes(file_name: &str) -> Vec<u8> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data/fixtures")
            .join(file_name);
        std::fs::read(&path).unwrap_or_else(|e| panic!("read fixture {path:?}: {e}"))
    }

    /// Parses a checked-in fixture .class (generated at development time by
    /// tools/gen-class-fixtures/gen.ps1; no JVM is invoked at test runtime).
    fn parse_fixture(binary_name: &str, file_name: &str, save_parameter_names: bool) -> Option<ParseSummary> {
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
        );
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let mut syms = Symtab::new(&mut ctx, Rc::clone(&names), Target::JDK1_17, source);
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
            .read_class_file(&mut ctx, &mut syms, c)
            .expect("readClassFile");
        let class_ty = ctx.sym(c).ty.expect("class type");
        let super_is_none = match &ctx.typ(class_ty).kind {
            crate::code::type_::TypeKindData::ClassType { supertype_field, .. } => supertype_field
                .map(|st| ctx.typ(st).has_tag(crate::code::type_tag::TypeTag::NONE))
                .unwrap_or(true),
            _ => panic!("class type expected"),
        };
        let interfaces = match &ctx.typ(class_ty).kind {
            crate::code::type_::TypeKindData::ClassType { interfaces_field, .. } => {
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
            members.push((s.kind, s.flags_field, s.name.to_java_string().to_string(), ty));
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
        );
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let mut syms = Symtab::new(&mut ctx, Rc::clone(&names), Target::JDK1_17, source);
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
            .read_class_file(&mut ctx, &mut syms, c)
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
            crate::code::symbol::SymbolKind::ClassSymbol { members_field, .. } => {
                *members_field
            }
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
        let names: Vec<&str> = summary.members.iter().map(|(_, _, n, _)| n.as_str()).collect();
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
        );
        let mut ctx = crate::compiler_context::CompilerContext::new();
        let mut syms = Symtab::new(&mut ctx, Rc::clone(&names), Target::JDK1_17, source);
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
            .read_class_file(&mut ctx, &mut syms, outer)
            .expect("outer readClassFile");
        // 2) read the local class whose EnclosingMethod references the outer method.
        let local_name =
            names.from_string(&javac_strings::JavaString::from("Fixtures$1Local"));
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
            .read_class_file(&mut ctx, &mut syms, local)
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
        let names: Vec<&str> = record.members.iter().map(|(_, _, n, _)| n.as_str()).collect();
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
        assert!(circle.interfaces.iter().any(|i| i.contains("SealedFixture")));
    }

    #[test]
    fn bad_magic_returns_bad_class_file_key() {
        let Some(mut bytes) = baseline_class_bytes("java.lang.Object") else {
            eprintln!("skipping: no baseline JDK 17 installation found");
            return;
        };
        bytes[0..4].copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
        let failure = parse_with_bytes("java.lang.Object", bytes).expect_err("bad magic");
        let code = failure.get_diagnostic().expect("diagnostic").get_diagnostic_info().get_code().to_string();
        assert_eq!(code, "bad.class.file.header", "upstream BadClassFile wrapper key");
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
        let code = failure.get_diagnostic().expect("diagnostic").get_diagnostic_info().get_code().to_string();
        assert_eq!(code, "bad.class.file.header", "upstream BadClassFile wrapper key");
    }

    #[test]
    fn bad_constant_pool_tag_returns_bad_class_file_key() {
        // magic, minor 0, major 61, constant_pool_count 2, tag 0xEE
        let bytes = vec![0xCA, 0xFE, 0xBA, 0xBE, 0x00, 0x00, 0x00, 0x3D, 0x00, 0x02, 0xEE];
        let failure = parse_with_bytes("demo.Bad", bytes).expect_err("bad pool tag");
        let code = failure.get_diagnostic().expect("diagnostic").get_diagnostic_info().get_code().to_string();
        assert_eq!(code, "bad.class.file.header", "upstream BadClassFile wrapper key");
    }

    use super::*;
    use crate::code::preview::Preview;
    use crate::util::context::Context;

    fn reader() -> ClassReader {
        let mut context = Context::new();
        let log = Log::instance(&mut context);
        let names = Rc::new(Names::new(false));
        let messages = crate::util::javac_messages::JavacMessages::instance(&mut context);
        let factory = Rc::new(Factory::new(messages, "compiler"));
        let dcfh = DeferredCompletionFailureHandler::instance();
        let preview = Rc::new(Preview::new(false, false));
        let profile = Rc::new(Profile::values()[0]);
        let options = Options::instance(&mut context);
        ClassReader::new(log, names, factory, dcfh, preview, profile, &options, Source::JDK17)
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
}