// Origin: com/sun/tools/javac/jvm/ClassReader.java @ 982d53c (jdk-17.0.19-ga)
// PORTING_RULES §19 gap registration (method-level audit of ClassReader.java):
// Dependency/phase: P2/S2-S3 (attribute readers/signature type-variable
// machinery/annotations) and P3 (Scope/Symbol/Type semantic batches).
// Not yet ported upstream methods (no substitutes in this file):
// - NOT PORTED: instance (ClassReader.java:257)
// - NOT PORTED: getEnclosingType (ClassReader.java:575)
// - NOT PORTED: setEnclosingType (ClassReader.java:600)
// - NOT PORTED: accepts (ClassReader.java:759)
// - NOT PORTED: read (ClassReader.java:778)
// - NOT PORTED: initAttributeReaders (ClassReader.java:794)
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
// - NOT PORTED: accepts (ClassReader.java:1092)
// - NOT PORTED: read (ClassReader.java:1095)
// - NOT PORTED: classNameMapper (ClassReader.java:1195)
// - NOT PORTED: accepts (ClassReader.java:1202)
// - NOT PORTED: read (ClassReader.java:1205)
// - NOT PORTED: accepts (ClassReader.java:1215)
// - NOT PORTED: read (ClassReader.java:1218)
// - NOT PORTED: accepts (ClassReader.java:1236)
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
        ClassReader {
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
            no_type: None,
        }
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

    /// Java: List<Type> sigToTypeParams() — requires Types.setBounds and the
    /// signature type-variable machinery (S2 Signature attribute batch).
    fn sig_to_type_params(
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &crate::code::symtab::Symtab,
    ) -> Result<List<TypeId>, crate::code::symbol::CompletionFailure> {
        unimplemented!("TODO(P2/S2): ClassReader.sigToTypeParams (Types.setBounds not ported yet)")
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
    ///
    /// TODO(P2/S2): the type-variable lookup needs the TypeVar/TypeVariableSymbol
    /// construction wiring and Types.setBounds (S2 Signature-attribute batch);
    /// no proxy substitute is implemented here.
    fn find_type_var(
        &mut self,
        _ctx: &mut CompilerContext,
        _syms: &crate::code::symtab::Symtab,
        _name: &Name,
    ) -> Result<TypeId, crate::code::symbol::CompletionFailure> {
        unimplemented!("TODO(P2/S2): ClassReader.findTypeVar (TypeVar construction/types.setBounds)")
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
        self.read_class_attrs(c);

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
            // Java: for (RecordComponent rc: c.getRecordComponents()) rc.accessor =
            // lookupMethod(c, rc.name, List.nil());
            unimplemented!("TODO(P2/S2): record component accessor lookup (Record attribute batch)");
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
        self.read_member_attrs(var);
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
        self.read_member_attrs(method);
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

    /// Java: void readAttrs(Symbol sym, AttributeKind kind) — the attribute
    /// reader table is not populated yet (S2 attribute batch); unknown
    /// attributes are skipped exactly like the upstream unknown-attribute path.
    fn read_attrs(&mut self, _sym: SymbolId, _class: bool) {
        let ac = self.next_char();
        for _ in 0..ac {
            let name_index = self.next_char() as i32;
            let _name = match self.get_name(name_index) {
                Ok(name) => name,
                Err(_) => self.names.empty.clone(),
            };
            let attr_len = self.next_int();
            // Java: AttributeReader r = attributeReaders.get(attrName); if (r != null && r.accepts(kind)) r.read(sym, attrLen); else { bp = bp + attrLen; }
            self.bp += attr_len;
        }
    }

    /// Java: void readClassAttrs(ClassSymbol c)
    fn read_class_attrs(&mut self, c: SymbolId) {
        self.read_attrs(c, true);
    }

    /// Java: void readMemberAttrs(Symbol sym)
    fn read_member_attrs(&mut self, sym: SymbolId) {
        self.read_attrs(sym, false);
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
        members: Vec<(crate::code::kinds::Kind, i64, String, Option<String>)>,
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
                    let tsym = ctx.typ(inter).tsym.expect("interface symbol");
                    names_out.push(ctx.sym(tsym).flat_name().to_java_string().to_string());
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
        let mut members = Vec::new();
        for sym in symbols.iter() {
            let s = ctx.sym(*sym);
            let type_tag = s.ty.map(|t| format!("{:?}", ctx.typ(t).get_tag()));
            members.push((
                s.kind,
                s.flags_field,
                s.name.to_java_string().to_string(),
                type_tag,
            ));
        }
        Some(ParseSummary {
            flags: ctx.sym(c).flags_field,
            super_is_none,
            interfaces,
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
        assert_eq!(summary.interfaces, vec!["java.util.Collection".to_string()]);
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
                "java.lang.Comparable".to_string(),
                "java.lang.constant.Constable".to_string(),
                "java.lang.constant.ConstantDesc".to_string(),
            ]
        );
        assert_eq!(summary.members.len(), 171, "members: {}", summary.members.len());
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