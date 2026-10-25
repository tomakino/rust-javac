// Origin: com/sun/tools/javac/code/ClassFinder.java @ 982d53c (jdk-17.0.19-ga)
// PORTING_RULES §19 gap registration (method-level audit of the ported upstream class):
// Dependency/phase: P2/S3-S4 (file layer: JavaFileManager/Locations/BaseFileManager/ct.sym; BadClassFile is already ported).
// Not yet ported upstream methods (no substitutes in this file):
// - NOT PORTED: getCompleter (ClassFinder.java:170)
// - NOT PORTED: instance (ClassFinder.java:175)
// - NOT PORTED: complete (ClassFinder.java:282)
// - NOT PORTED: completeOwners (ClassFinder.java:314)
// - NOT PORTED: completeEnclosing (ClassFinder.java:325)
// - NOT PORTED: classFileNotFound (ClassFinder.java:394)
// - NOT PORTED: newCompletionFailure (ClassFinder.java:402)
// - NOT PORTED: loadClass (ClassFinder.java:422)
// - NOT PORTED: includeClassFile (ClassFinder.java:455)
// - NOT PORTED: preferredFileObject (ClassFinder.java:497)
// - NOT PORTED: getPackageFileKinds (ClassFinder.java:515)
// - NOT PORTED: extraFileActions (ClassFinder.java:522)
// - NOT PORTED: fillIn (ClassFinder.java:534)
// - NOT PORTED: scanModulePaths (ClassFinder.java:567)
// - NOT PORTED: scanUserPaths (ClassFinder.java:629)
// - NOT PORTED: scanPlatformPath (ClassFinder.java:697)
// - NOT PORTED: fillIn (ClassFinder.java:706)
// - NOT PORTED: hasNext (ClassFinder.java:745)
// - NOT PORTED: next (ClassFinder.java:765)
// End of §19 gap registration.

//! ClassFinder (P2/S3 batch): this file currently ports only the
//! ClassFinder.BadClassFile / BadEnclosingMethodAttr error types required by
//! ClassReader's badClassFile channel. The ClassFinder body (fillIn/complete/
//! finder state, class path scanning) is NOT ported yet: it depends on the file
//! layer (JavaFileManager/Locations/BaseFileManager) and is registered for the
//! S3/S4 batch (no substitute implementation is provided here).
//!
//! PORTING_RULES adapters (registered):
//! - Java BadClassFile extends CompletionFailure -> Rust composition
//!   (BadClassFile.failure); BadEnclosingMethodAttr wraps BadClassFile and
//!   derefs to it (Rust has no class inheritance).
//! - createBadClassFileDiagnostic's diagFactory.fragment(key, file, diag) uses
//!   the P4 placeholder arguments (DiagnosticArg text) until the diagnostic
//!   layer replaces them (ADR 0002).

use std::rc::Rc;

use javac_tools::tools::java_file_object::{JavaFileObject, Kind};

use crate::code::deferred_completion_failure_handler::DeferredCompletionFailureHandler;
use crate::code::symbol::CompletionFailure;
use crate::ids::SymbolId;
use crate::resources::compiler_properties::fragments;
use crate::resources::diagnostic_contract::DiagnosticArg;
use crate::util::jc_diagnostic::{Factory, Fragment, JCDiagnostic};

/// Used for bad class definition files, such as bad .class files or
/// for .java files with unexpected package or class names.
///
/// Java: public static class ClassFinder.BadClassFile extends CompletionFailure
pub struct BadClassFile {
    /// Java: the inherited CompletionFailure part.
    pub failure: CompletionFailure,
}

impl BadClassFile {
    /// Java: BadClassFile(TypeSymbol sym, JavaFileObject file, JCDiagnostic diag,
    /// JCDiagnostic.Factory diagFactory, DeferredCompletionFailureHandler dcfh)
    pub fn new(
        sym: Option<SymbolId>,
        file: Rc<dyn JavaFileObject>,
        diag: JCDiagnostic,
        diag_factory: Rc<Factory>,
        dcfh: Option<Rc<DeferredCompletionFailureHandler>>,
    ) -> BadClassFile {
        let supplier_file = Rc::clone(&file);
        let supplier_diag = diag.clone();
        let supplier_factory = Rc::clone(&diag_factory);
        let failure = CompletionFailure::new(
            sym,
            Box::new(move || {
                create_bad_class_file_diagnostic(
                    &supplier_file,
                    &supplier_diag,
                    &supplier_factory,
                )
            }),
            dcfh,
        );
        BadClassFile { failure }
    }
}

/// Java: private static JCDiagnostic createBadClassFileDiagnostic(
/// JavaFileObject file, JCDiagnostic diag, JCDiagnostic.Factory diagFactory)
pub fn create_bad_class_file_diagnostic(
    file: &Rc<dyn JavaFileObject>,
    diag: &JCDiagnostic,
    diag_factory: &Rc<Factory>,
) -> JCDiagnostic {
    let is_source = file.get_kind() == Kind::SOURCE;
    // PORTING_RULES: P4 placeholder arguments (ADR 0002); the file and the
    // nested diagnostic are carried as their text until the typed diagnostic
    // arguments land.
    let file_arg = DiagnosticArg::from_display(file.to_uri());
    let diag_arg = DiagnosticArg::from_display(diag.get_message(None));
    let fragment = if is_source {
        Fragment::from(fragments::bad_source_file_header(file_arg, diag_arg))
    } else {
        Fragment::from(fragments::bad_class_file_header(file_arg, diag_arg))
    };
    diag_factory.fragment(fragment)
}

/// Java: public static class ClassFinder.BadEnclosingMethodAttr extends BadClassFile
pub struct BadEnclosingMethodAttr {
    pub bad_class_file: BadClassFile,
}

impl BadEnclosingMethodAttr {
    /// Java: BadEnclosingMethodAttr(TypeSymbol sym, JavaFileObject file,
    /// JCDiagnostic diag, JCDiagnostic.Factory diagFactory, DeferredCompletionFailureHandler dcfh)
    pub fn new(
        sym: Option<SymbolId>,
        file: Rc<dyn JavaFileObject>,
        diag: JCDiagnostic,
        diag_factory: Rc<Factory>,
        dcfh: Option<Rc<DeferredCompletionFailureHandler>>,
    ) -> BadEnclosingMethodAttr {
        BadEnclosingMethodAttr {
            bad_class_file: BadClassFile::new(sym, file, diag, diag_factory, dcfh),
        }
    }
}

impl std::ops::Deref for BadEnclosingMethodAttr {
    type Target = BadClassFile;

    fn deref(&self) -> &BadClassFile {
        &self.bad_class_file
    }
}
