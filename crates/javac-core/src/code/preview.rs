// Origin: com/sun/tools/javac/code/Preview.java @ 982d53c (jdk-17.0.19-ga)

//! Helper class to handle preview language features. This class maps certain language
//! features (see {@link Feature}) into 'preview' features; the mapping is completely
//! ad-hoc, so as to allow for maximum flexibility, which allows to migrate preview
//! feature into supported features with ease.
//!
//! This class acts as a centralized point against which usages of preview features are
//! reported by clients (e.g. other javac classes). Internally, this class collects all
//! such usages and generates diagnostics to inform the user of such usages. Such
//! diagnostics can be enabled using the {@link LintCategory#PREVIEW} lint category, and
//! are suppressible by usual means.
//!
//! PORTING_RULES:
//! * Java class -> Rust struct; instance(Context) is the Java static factory.
//! * sourcesWithPreviewFeatures is a Java HashSet<JavaFileObject> relying on
//!   JavaFileObject's identity equals/hashCode; Rust has no implicit identity hash,
//!   so the set is a Vec<JavaFileObjectRef> searched with Rc::ptr_eq (same identity
//!   semantics; the set is tiny).  Map<Integer, Source> -> HashMap<i32, Source>.
//! * the Java overloads of warnPreview/disabledError become distinct Rust names
//!   (§3): warn_preview(int, Feature) -> warn_preview, warnPreview(
//!   DiagnosticPosition, Feature) -> warn_preview_at, warnPreview(JavaFileObject,
//!   int) -> warn_preview_classfile; disabledError(Feature) -> disabled_error,
//!   disabledError(JavaFileObject, int) -> disabled_error_classfile.
//! * TODO(Log/Lint/Options batch): the constructor reads Options (enabled,
//!   forcePreview), Log.instance, Lint.instance and creates a
//!   MandatoryWarningHandler; none of these is ported yet.  instance(Context) and the
//!   diagnostics entry points that need them (warnPreview x3, markUsesPreview,
//!   reportPreviewWarning, reportDeferredDiagnostics, clear, checkSourceLevel) are
//!   unimplemented (no substitute Log/Options/MandatoryWarningHandler).  The fields
//!   used only by those methods (log, lint, previewHandler) are not represented yet.
//! * the constructor adapter new(enabled, force_preview) takes the two option flags
//!   directly (the full Java constructor wiring lands with the Options/Log batch);
//!   majorVersionToSource is initialized by the ported private method.
//! * disabledError builds the same resource keys as upstream
//!   (Errors.PreviewFeatureDisabled[Plural]/PreviewFeatureDisabledClassfile); the
//!   nested fragment / file object diagnostic arguments use the current resources
//!   contract display text (ADR 0002 revision C, replaced at P4).

use std::collections::HashMap;
use std::rc::Rc;

use javac_tools::tools::java_file_manager::JavaFileObjectRef;
use javac_tools::tools::java_file_object::JavaFileObject;

use crate::code::source::{Feature, Source};
use crate::jvm::target::Target;
use crate::resources::compiler_properties::errors;
use crate::resources::diagnostic_contract::{DiagnosticArg, FileObjectArg};
use crate::util::assert;
use crate::util::context::Context;
use crate::util::java_string::JavaString;
use crate::util::jc_diagnostic::{DiagnosticPosition, Error, Warning};

/// Helper class to handle preview language features.
pub struct Preview {
    /// flag: are preview features enabled
    /// Java: private final boolean enabled;
    enabled: bool,

    /// test flag: should all features be considered as preview features?
    /// Java: private final boolean forcePreview;
    force_preview: bool,

    /// a mapping from classfile numbers to Java SE versions
    /// Java: private final Map<Integer, Source> majorVersionToSource;
    major_version_to_source: HashMap<i32, Source>,

    /// Java: private final Set<JavaFileObject> sourcesWithPreviewFeatures = new HashSet<>();
    sources_with_preview_features: Vec<JavaFileObjectRef>,

    // Java: private final MandatoryWarningHandler previewHandler;
    // Java: private final Lint lint;  private final Log log;
    // TODO(Log/Lint batch): those dependencies are not ported yet; the fields are
    // represented once the Log/Lint construction is available.
}

impl Preview {
    /// Adapter constructor for the ported state: takes the two option flags
    /// instead of reading Options (not ported yet).
    ///
    /// Java: Preview(Context context) — the full constructor reads Options
    /// (PREVIEW/"forcePreview"), Log.instance, Lint.instance and creates the
    /// MandatoryWarningHandler; those dependencies are not ported yet.
    // TODO(Log/Lint/Options batch): the P6 wiring builds Preview from the ported
    // Context (Options/Log/Lint); until then this adapter constructor is used by the
    // module tests, so it is allow(dead_code) for non-test builds.
    #[allow(dead_code)]
    pub(crate) fn new(enabled: bool, force_preview: bool) -> Preview {
        Preview {
            enabled,
            force_preview,
            major_version_to_source: Preview::init_major_version_to_source_map(),
            sources_with_preview_features: Vec::new(),
        }
    }

    /// Java:
    /// <pre>
    /// private Map<Integer, Source> initMajorVersionToSourceMap() {
    ///     Map<Integer, Source> majorVersionToSource = new HashMap<>();
    ///     for (Target t : Target.values()) {
    ///         int major = t.majorVersion;
    ///         Source source = Source.lookup(t.name);
    ///         if (source != null) {
    ///             majorVersionToSource.put(major, source);
    ///         }
    ///     }
    ///     return majorVersionToSource;
    /// }
    /// </pre>
    fn init_major_version_to_source_map() -> HashMap<i32, Source> {
        let mut major_version_to_source = HashMap::new();
        for target in Target::values() {
            let major = target.major_version();
            if let Some(source) = Source::lookup(target.name()) {
                major_version_to_source.insert(major, source);
            }
        }
        major_version_to_source
    }

    /// Java:
    /// <pre>
    /// public static Preview instance(Context context) {
    ///     Preview instance = context.get(previewKey);
    ///     if (instance == null) {
    ///         instance = new Preview(context);
    ///     }
    ///     return instance;
    /// }
    /// </pre>
    ///
    /// TODO(Log/Lint/Options batch): the constructor reads Options/Log/Lint; the
    /// entry point stays unimplemented (no substitute components).
    pub fn instance(_context: &mut Context) -> Rc<Preview> {
        unimplemented!(
            "TODO(Log/Lint/Options batch): Preview.instance (Options/Log/Lint/MandatoryWarningHandler are not ported)"
        )
    }

    /// Report usage of a preview feature. Usages reported through this method will
    /// affect the set of sourcefiles with dependencies on preview features.
    /// @param pos the position at which the preview feature was used.
    /// @param feature the preview feature used.
    ///
    /// Java: public void warnPreview(int pos, Feature feature).
    pub fn warn_preview(&self, _pos: i32, _feature: Feature) {
        unimplemented!(
            "TODO(Log/Lint batch): Preview.warnPreview (Log.currentSourceFile and MandatoryWarningHandler are not ported)"
        )
    }

    /// Report usage of a preview feature. Usages reported through this method will
    /// affect the set of sourcefiles with dependencies on preview features.
    /// @param pos the position at which the preview feature was used.
    /// @param feature the preview feature used.
    ///
    /// Java: public void warnPreview(DiagnosticPosition pos, Feature feature).
    pub fn warn_preview_at(&self, _pos: &dyn DiagnosticPosition, _feature: Feature) {
        unimplemented!(
            "TODO(Log/Lint batch): Preview.warnPreview (Log.currentSourceFile and MandatoryWarningHandler are not ported)"
        )
    }

    /// Report usage of a preview feature in classfile.
    /// @param classfile the name of the classfile with preview features enabled
    /// @param major_version the major version found in the classfile.
    ///
    /// Java: public void warnPreview(JavaFileObject classfile, int majorVersion).
    pub fn warn_preview_classfile(&self, _classfile: &dyn JavaFileObject, _major_version: i32) {
        unimplemented!(
            "TODO(Log/Lint batch): Preview.warnPreview(JavaFileObject, int) (Log/MandatoryWarningHandler are not ported)"
        )
    }

    /// Java: public void markUsesPreview(DiagnosticPosition pos).
    pub fn mark_uses_preview(&self, _pos: &dyn DiagnosticPosition) {
        unimplemented!(
            "TODO(Log batch): Preview.markUsesPreview (Log.currentSourceFile is not ported)"
        )
    }

    /// Java: public void reportPreviewWarning(DiagnosticPosition pos, Warning warnKey).
    pub fn report_preview_warning(&self, _pos: &dyn DiagnosticPosition, _warn_key: Warning) {
        unimplemented!(
            "TODO(Log batch): Preview.reportPreviewWarning (MandatoryWarningHandler is not ported)"
        )
    }

    /// Java: public boolean usesPreview(JavaFileObject file) {
    ///           return sourcesWithPreviewFeatures.contains(file); }
    pub fn uses_preview(&self, file: &JavaFileObjectRef) -> bool {
        self.sources_with_preview_features
            .iter()
            .any(|candidate| Rc::ptr_eq(candidate, file))
    }

    /// Are preview features enabled?
    /// @return true, if preview features are enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Is given feature a preview feature?
    /// @param feature the feature to be tested.
    /// @return true, if given feature is a preview feature.
    pub fn is_preview(&self, feature: Feature) -> bool {
        match feature {
            Feature::CASE_NULL => true,
            Feature::PATTERN_SWITCH => true,
            // Note: this is a backdoor which allows to optionally treat all features
            // as 'preview' (for testing).  When real preview features will be added,
            // this method can be implemented to return 'true' for those selected
            // features, and 'false' for all the others.
            _ => self.force_preview,
        }
    }

    /// Generate an error key which captures the fact that a given preview feature
    /// could not be used due to the preview feature support being disabled.
    /// @param feature the feature for which the diagnostic has to be generated.
    /// @return the diagnostic.
    pub fn disabled_error(&self, feature: Feature) -> Error {
        assert::check(!self.is_enabled());
        // ADR 0002 revision C temporary representation: the fragment argument is
        // passed by its diagnostic key until the P4 contract lands.
        let arg = DiagnosticArg::Text(feature.name_fragment().info().key());
        if feature.is_plural() {
            errors::preview_feature_disabled_plural(arg).into()
        } else {
            errors::preview_feature_disabled(arg).into()
        }
    }

    /// Generate an error key which captures the fact that a preview classfile cannot
    /// be loaded due to the preview feature support being disabled.
    /// @param classfile the name of the classfile with preview features enabled
    /// @param major_version the major version found in the classfile.
    pub fn disabled_error_classfile(&self, classfile: &dyn JavaFileObject, major_version: i32) -> Error {
        assert::check(!self.is_enabled());
        // Java: majorVersionToSource.get(majorVersion).name -> NullPointerException
        // when the map has no entry for the major version.
        let source = match self.major_version_to_source.get(&major_version) {
            Some(source) => *source,
            None => panic!(
                "NullPointerException: Preview.majorVersionToSource.get({major_version}) is null"
            ),
        };
        // ADR 0002 revision C temporary representation (file object argument).
        let file_arg = FileObjectArg::from(classfile.get_name().to_string_lossy().as_str());
        errors::preview_feature_disabled_classfile(file_arg, JavaString::from(source.name())).into()
    }

    /// Check whether the given symbol has been declared using
    /// a preview language feature.
    ///
    /// @param sym Symbol to check
    /// @return true iff sym has been declared using a preview language feature
    pub fn declared_using_preview_feature(&self, _sym: &crate::code::symbol::Symbol) -> bool {
        false
    }

    /// Report any deferred diagnostics.
    pub fn report_deferred_diagnostics(&self) {
        unimplemented!(
            "TODO(Log batch): Preview.reportDeferredDiagnostics (MandatoryWarningHandler is not ported)"
        )
    }

    /// Java: public void clear().
    pub fn clear(&self) {
        unimplemented!(
            "TODO(Log batch): Preview.clear (MandatoryWarningHandler is not ported)"
        )
    }

    /// Java: public void checkSourceLevel(DiagnosticPosition pos, Feature feature).
    pub fn check_source_level(&self, _pos: &dyn DiagnosticPosition, _feature: Feature) {
        unimplemented!(
            "TODO(Log/Lint batch): Preview.checkSourceLevel (Log.error and Lint are not ported)"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::Preview;
    use crate::code::source::{Feature, Source};
    use crate::util::java_string::JavaString;
    use crate::util::jc_diagnostic::{DiagnosticType, SimpleDiagnosticPosition};

    #[test]
    fn is_enabled_reflects_the_flag() {
        assert!(!Preview::new(false, false).is_enabled());
        assert!(Preview::new(true, false).is_enabled());
    }

    #[test]
    fn is_preview_checks_the_hardcoded_features_and_the_force_flag() {
        let preview = Preview::new(false, false);
        assert!(preview.is_preview(Feature::CASE_NULL));
        assert!(preview.is_preview(Feature::PATTERN_SWITCH));
        assert!(!preview.is_preview(Feature::DIAMOND));
        assert!(!preview.is_preview(Feature::SEALED_CLASSES));
        let forced = Preview::new(false, true);
        assert!(forced.is_preview(Feature::DIAMOND));
        assert!(forced.is_preview(Feature::RECORDS));
    }

    #[test]
    fn major_version_map_is_derived_from_target_and_source() {
        let preview = Preview::new(false, false);
        let map = &preview.major_version_to_source;
        // Target.JDK1_1 has no Source counterpart (1.1 could not pass the JCK),
        // all other 16 targets map to their source level.
        assert_eq!(map.len(), 16);
        assert_eq!(map.get(&45), None);
        assert_eq!(map.get(&46), Some(&Source::JDK1_2));
        assert_eq!(map.get(&49), Some(&Source::JDK5));
        assert_eq!(map.get(&52), Some(&Source::JDK8));
        assert_eq!(map.get(&61), Some(&Source::JDK17));
    }

    #[test]
    fn disabled_error_builds_the_normal_and_plural_error_keys() {
        let preview = Preview::new(false, false);
        let error = preview.disabled_error(Feature::CASE_NULL);
        assert_eq!(error.info().get_type(), DiagnosticType::ERROR);
        assert_eq!(error.info().get_code(), "preview.feature.disabled");
        let args = error.info().get_args();
        assert_eq!(args.len(), 1);
        assert_eq!(
            args[0]
                .downcast_ref::<JavaString>()
                .expect("java string argument")
                .to_string_lossy(),
            "compiler.misc.feature.case.null"
        );

        let plural = preview.disabled_error(Feature::SEALED_CLASSES);
        assert_eq!(plural.info().get_code(), "preview.feature.disabled.plural");
    }

    #[test]
    #[should_panic]
    fn disabled_error_requires_preview_to_be_disabled() {
        // Java: Assert.check(!isEnabled());
        let preview = Preview::new(true, false);
        let _ = preview.disabled_error(Feature::CASE_NULL);
    }

    #[test]
    fn disabled_error_classfile_uses_the_major_version_source() {
        let preview = Preview::new(false, false);
        let file = TestJavaFileObject;
        let error = preview.disabled_error_classfile(&file, 61);
        assert_eq!(error.info().get_type(), DiagnosticType::ERROR);
        assert_eq!(error.info().get_code(), "preview.feature.disabled.classfile");
        let args = error.info().get_args();
        assert_eq!(args.len(), 2);
        assert_eq!(
            args[0]
                .downcast_ref::<JavaString>()
                .expect("java string argument")
                .to_string_lossy(),
            "Foo.class"
        );
        assert_eq!(
            args[1]
                .downcast_ref::<JavaString>()
                .expect("java string argument")
                .to_string_lossy(),
            "17"
        );
    }

    #[test]
    #[should_panic(expected = "NullPointerException")]
    fn disabled_error_classfile_panics_for_an_unknown_major_version() {
        // Java: majorVersionToSource.get(majorVersion).name -> NullPointerException.
        let preview = Preview::new(false, false);
        let _ = preview.disabled_error_classfile(&TestJavaFileObject, 45);
    }

    struct TestJavaFileObject;

    impl javac_tools::tools::file_object::FileObject for TestJavaFileObject {
        fn to_uri(&self) -> javac_tools::uri::URI {
            unimplemented!("test fixture")
        }
        fn get_name(&self) -> JavaString {
            JavaString::from("Foo.class")
        }
        fn open_input_stream(&self) -> Result<Box<dyn std::io::Read>, javac_tools::error::Error> {
            unimplemented!("test fixture")
        }
        fn open_output_stream(&self) -> Result<Box<dyn std::io::Write>, javac_tools::error::Error> {
            unimplemented!("test fixture")
        }
        fn open_reader(
            &self,
            _ignore_encoding_errors: bool,
        ) -> Result<Box<dyn javac_tools::java_io::Reader>, javac_tools::error::Error> {
            unimplemented!("test fixture")
        }
        fn get_char_content(
            &self,
            _ignore_encoding_errors: bool,
        ) -> Result<Option<JavaString>, javac_tools::error::Error> {
            unimplemented!("test fixture")
        }
        fn open_writer(
            &self,
        ) -> Result<Box<dyn javac_tools::java_io::Writer>, javac_tools::error::Error> {
            unimplemented!("test fixture")
        }
        fn get_last_modified(&self) -> i64 {
            0
        }
        fn delete(&self) -> bool {
            false
        }
    }

    impl javac_tools::tools::java_file_object::JavaFileObject for TestJavaFileObject {
        fn get_kind(&self) -> javac_tools::tools::java_file_object::Kind {
            javac_tools::tools::java_file_object::Kind::CLASS
        }
        fn is_name_compatible(
            &self,
            _simple_name: &JavaString,
            _kind: javac_tools::tools::java_file_object::Kind,
        ) -> bool {
            false
        }
        fn get_nesting_kind(&self) -> Option<javac_tools::lang::model::element::NestingKind> {
            None
        }
        fn get_access_level(&self) -> Option<javac_tools::lang::model::element::Modifier> {
            None
        }
    }

    #[test]
    fn uses_preview_compares_file_object_identity() {
        let mut preview = Preview::new(false, false);
        let first: javac_tools::tools::java_file_manager::JavaFileObjectRef =
            std::rc::Rc::new(TestJavaFileObject);
        let second: javac_tools::tools::java_file_manager::JavaFileObjectRef =
            std::rc::Rc::new(TestJavaFileObject);
        assert!(!preview.uses_preview(&first));
        preview.sources_with_preview_features.push(first.clone());
        assert!(preview.uses_preview(&first));
        // JavaFileObject has no equals override: a different instance is a
        // different set element.
        assert!(!preview.uses_preview(&second));
    }

    #[test]
    #[should_panic(expected = "TODO(Log batch)")]
    fn mark_uses_preview_needs_the_log_port() {
        let preview = Preview::new(false, false);
        let pos = SimpleDiagnosticPosition::new(0);
        preview.mark_uses_preview(&pos);
    }

    #[test]
    #[should_panic(expected = "TODO(Log/Lint/Options batch)")]
    fn instance_needs_the_log_lint_options_port() {
        let mut context = crate::util::context::Context::new();
        let _ = Preview::instance(&mut context);
    }
}
