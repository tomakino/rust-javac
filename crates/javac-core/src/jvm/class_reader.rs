// Origin: com/sun/tools/javac/jvm/ClassReader.java @ 982d53c (jdk-17.0.19-ga)
// PORTING_RULES §19 gap registration (method-level audit of the ported upstream class):
// Dependency/phase: P2/S2-S3 (attribute readers/signature state machine/annotations) and P3 (Scope.WriteableScope/Types.subst/Annotate/ClassFinder wiring).
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
// - NOT PORTED: enterTypevars (ClassReader.java:2763)
// - NOT PORTED: enterClass (ClassReader.java:2776)
// - NOT PORTED: enterClass (ClassReader.java:2780)
// - NOT PORTED: lookupMethod (ClassReader.java:2871)
// - NOT PORTED: toUri (ClassReader.java:3071)
// - NOT PORTED: getName (ClassReader.java:3080)
// - NOT PORTED: getKind (ClassReader.java:3085)
// - NOT PORTED: openInputStream (ClassReader.java:3090)
// - NOT PORTED: openOutputStream (ClassReader.java:3095)
// - NOT PORTED: getCharContent (ClassReader.java:3100)
// - NOT PORTED: openReader (ClassReader.java:3105)
// - NOT PORTED: openWriter (ClassReader.java:3110)
// - NOT PORTED: getLastModified (ClassReader.java:3115)
// - NOT PORTED: delete (ClassReader.java:3120)
// - NOT PORTED: isNameCompatible (ClassReader.java:3125)
// - NOT PORTED: getNestingKind (ClassReader.java:3130)
// - NOT PORTED: getAccessLevel (ClassReader.java:3135)
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
use crate::ids::SymbolId;
use crate::jvm::class_file::{Version, JAVA_MAGIC, PREVIEW_MINOR_VERSION};
use crate::jvm::pool_reader::PoolReader;
use crate::jvm::profile::Profile;
use crate::resources::compiler_properties::warnings;
use crate::resources::diagnostic_contract::DiagnosticArg;
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
}

impl ClassReader {
    /// Java: public static final int INITIAL_BUFFER_SIZE = 0x0fff0;
    pub const INITIAL_BUFFER_SIZE: i32 = 0x0fff0;

    /// Java: public static ClassReader instance(Context context) /
    /// protected ClassReader(Context context)
    ///
    /// PORTING_RULES: the Context singletons (Annotate/Types/Symtab/
    /// JavaFileManager/JCDiagnostic.Factory/DeferredCompletionFailureHandler/
    /// Options/Source/Preview/Profile/WriteableScope/Lint) are injected instead;
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
            signature: Vec::new(),
            sigp: 0,
            siglimit: 0,
            sig_enter_phase: false,
            warned_attrs: HashSet::new(),
            filling: false,
            missing_type_variables: List::nil(),
            found_type_variables: List::nil(),
        }
    }

    /// Java: private void enterMember(ClassSymbol c, Symbol sym)
    ///
    /// TODO(P2/S2): wired with readClass/readField/readMethod; Scope.WriteableScope
    /// is not ported yet.
    #[allow(dead_code)]
    fn enter_member(
        &mut self,
        _ctx: &mut CompilerContext,
        _c: SymbolId,
        _sym: SymbolId,
    ) {
        unimplemented!("TODO(P2/S2): ClassReader.enterMember (Scope.WriteableScope is not ported yet)")
    }

    /// Java: public ClassFinder.BadClassFile badClassFile(String key, Object... args)
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

    /// Java: public void readClassFile(ClassSymbol c)
    pub fn read_class_file(
        &mut self,
        ctx: &mut CompilerContext,
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
            Err(ex) => {
                let diag = self
                    .diag_factory
                    .fragment(Fragment::from(crate::resources::compiler_properties::fragments::unable_to_access_file()));
                let _ = &ex;
                let failure_diag = diag.clone();
                return Err(crate::code::symbol::CompletionFailure::new(
                    Some(c),
                    Box::new(move || failure_diag.clone()),
                    Some(Rc::clone(&self.dcfh)),
                ));
            }
        };
        if let Err(ex) = self.buf.append_stream(&mut stream) {
            let _ = ex;
            let diag = self
                .diag_factory
                .fragment(Fragment::from(crate::resources::compiler_properties::fragments::unable_to_access_file()));
            let failure_diag = diag.clone();
            return Err(crate::code::symbol::CompletionFailure::new(
                Some(c),
                Box::new(move || failure_diag.clone()),
                Some(Rc::clone(&self.dcfh)),
            ));
        }
        self.read_class_buffer(ctx, c)?;
        // Java: the tail of readClassFile (missingTypeVariables substitution,
        // AnnotationTypeMetadata, module uses/provides) needs Types.subst/
        // Annotate/ModuleSymbol state that are not ported yet (S2/S3).
        unimplemented!("TODO(P2/S2): readClassFile tail (typevars subst/AnnotationTypeMetadata/module uses-provides)")
    }

    /// Java: private void readClassBuffer(ClassSymbol c) throws IOException
    pub fn read_class_buffer(
        &mut self,
        ctx: &mut CompilerContext,
        c: SymbolId,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        let magic = self.next_int();
        if magic != JAVA_MAGIC {
            let key = Fragment::from(crate::resources::compiler_properties::fragments::illegal_start_of_class_file());
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
                let warning = warnings::big_major_version(
                    DiagnosticArg::from_display("<class file>"),
                    self.major_version,
                    max_major,
                );
                let _ = warning;
                unimplemented!("TODO(P2/Log): log.warning(BigMajorVersion) needs a DiagnosticPosition-carrying Log adapter");
            } else {
                // Java: badClassFile("wrong.version", major, minor, maxMajor, maxMinor);
                // The generated accessor currently has no parameters (registered
                // P4 diagnostic-contract deviation, see the batch report).
                let key = Fragment::from(crate::resources::compiler_properties::fragments::wrong_version());
                return Err(self.bad_class_file(key).failure);
            }
        }
        if preview_class_file {
            if !self.preview.is_enabled() {
                // Java: log.error(preview.disabledError(currentClassFile, majorVersion));
                unimplemented!("TODO(P2/Log): preview.disabledError(JavaFileObject,int) wiring is unimplemented (Log batch)");
            } else {
                unimplemented!("TODO(P2/Log): preview.warnPreview(JavaFileObject,int) wiring is unimplemented (Log batch)");
            }
        }
        let mut pool_reader = PoolReader::new();
        let pool_result = pool_reader.read_pool(&self.buf, self.bp);
        let end = match pool_result {
            Ok(end) => end,
            Err(err) => {
                let _ = err;
                let key = Fragment::from(crate::resources::compiler_properties::fragments::bad_class_file(self.names.empty.clone()));
                return Err(self.bad_class_file(key).failure);
            }
        };
        self.bp = end;
        if self.signature_buffer.len() < self.bp as usize {
            let ns = (self.bp as usize).next_power_of_two();
            self.signature_buffer = vec![0u8; ns];
        }
        self.pool_reader = Some(pool_reader);
        self.read_class(ctx, c)
    }

    /// Java: void readClass(ClassSymbol c)
    fn read_class(
        &mut self,
        _ctx: &mut CompilerContext,
        _c: SymbolId,
    ) -> Result<(), crate::code::symbol::CompletionFailure> {
        unimplemented!("TODO(P2/S2): ClassReader.readClass needs Scope.WriteableScope/readField/readMethod/sigToType (S2 batch)")
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

    /// Java: public ClassFinder.BadEnclosingMethodAttr badEnclosingMethod(Symbol sym)
    ///
    /// TODO(P2/S2): Fragments.BadEnclosingMethod(sym) accessor needs the Symbol
    /// argument (P4 diagnostic contract).
    #[allow(dead_code)]
    pub fn bad_enclosing_method(
        &self,
        _sym: SymbolId,
    ) -> crate::code::class_finder::BadEnclosingMethodAttr {
        unimplemented!("TODO(P2/S2): Fragments.BadEnclosingMethod(sym) accessor needs the Symbol argument (P4 contract)")
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