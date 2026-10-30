// Origin: com/sun/tools/javac/util/AbstractDiagnosticFormatter.java @ 982d53c (jdk-17.0.19-ga)
// Gap registration supplement (method-level audit):
// Dependency/phase: P2/P3 utility-service.
// - NOT PORTED: expr2String (AbstractDiagnosticFormatter.java:231)
// - NOT PORTED: getConfiguration (AbstractDiagnosticFormatter.java:400)
// - NOT PORTED: getPrinter (AbstractDiagnosticFormatter.java:517)
// - NOT PORTED: setPrinter (AbstractDiagnosticFormatter.java:521)
// - NOT PORTED: capturedVarId (AbstractDiagnosticFormatter.java:541)
// - NOT PORTED: visitCapturedType (AbstractDiagnosticFormatter.java:545)
// End of Gap registration supplement.

//! This abstract class provides a basic implementation of the functionalities
//! that should be provided by any formatter used by javac. Among the main
//! features provided by AbstractDiagnosticFormatter are:
//!
//! <ul>
//! <li> Provides a standard implementation of the visitor-like methods defined
//! in the interface DiagnosticFormatter. Those implementations are specifically
//! targeting JCDiagnostic objects.
//! <li> Provides basic support for i18n and a method for executing all
//! locale-dependent conversions
//! <li> Provides the formatting logic for rendering the arguments of a
//! JCDiagnostic object.
//! </ul>
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES:
//! - the Java abstract class becomes a trait with default methods for the
//!   concrete methods; the upstream protected fields are provided by the
//!   implementor through accessor methods (messages / depth via Cell /
//!   configuration view); the two virtual calls used by the copied algorithms
//!   (formatDiagnostic and formatMessage) become the required trait methods
//!   format_diagnostic_impl / format_message_impl;
//! - method names that would collide with the DiagnosticFormatter trait methods
//!   carry an _impl suffix (Rust has no overriding); the other upstream methods
//!   keep the camelCase -> snake_case name;
//! - SimpleConfiguration(Options) needs the not-yet-ported Options: it is not
//!   provided (REGISTERED in the that ports Options); the report-style option
//!   decoding is registered in the module doc of the Options/Target;
//! - NOT PORTED IN THIS STEP (dependencies are not ported yet; REGISTERED, no
//!   substitute logic):
//!   * the Printer-based Type/Symbol argument formatting needs code/Printer
//!     (not ported);
//!   * the JCExpression argument formatting needs the tree arena context and
//!     Pretty.toSimpleString (the JCExpression args of the arena port are
//!     TreeId values; a formatter has no CompilerContext yet);
//!   * the PathFileObject short-name support (formatSource with
//!     fullname = false) needs file/PathFileObject (not ported);
//!   * Profile / Option / Target / Source argument formatting needs jvm/Target,
//!     jvm/Profile, main/Option and code/Source (not ported);
//!   * the captured-type Printer (allCaptured) needs code/Printer;
//!   * Tag arguments are ported (tree::jc_tree::Tag) and formatted with the
//!     compiler.misc.tree.tag.<name> message.

use std::rc::Rc;

use javac_tools::tools::java_file_object::JavaFileObject;

use crate::api::diagnostic_formatter::{
    Configuration, DiagnosticPart, MultilineLimit, PositionKind,
};
use crate::api::formattable::Formattable;
use crate::tree::jc_tree::Tag;
use crate::util::java_string::JavaString;
use crate::util::javac_messages::{JavacMessages, Locale};
use crate::util::jc_diagnostic::{DiagnosticArg, DiagnosticType, JCDiagnostic};
use crate::util::options::Options;
use crate::util::position::NOPOS;

/// Java: public abstract class AbstractDiagnosticFormatter implements
/// DiagnosticFormatter<JCDiagnostic>
pub trait AbstractDiagnosticFormatter {
    /// Java: protected JavacMessages messages (null for the raw formatter).
    fn messages(&self) -> Option<&Rc<JavacMessages>>;

    /// Java: protected int depth (mutable during formatting; a Cell because the
    /// copied algorithms format through shared references).
    fn depth(&self) -> i32;

    /// Java: depth++ / depth-- write-back.
    fn set_depth(&self, depth: i32);

    // Java: private SimpleConfiguration config - the parts used by the concrete
    // methods are exposed through these accessors (the full configuration is
    // owned by the implementor).
    fn config_visible(&self) -> Vec<DiagnosticPart>;
    fn config_multiline_limit(&self, limit: MultilineLimit) -> i32;
    fn config_caret_enabled(&self) -> bool;

    /// RichDiagnosticFormatter hook: render a type argument after replacing
    /// captured/intersection types with the CAP#N/INT#N names assigned by the
    /// formatter (Basic formatter keeps the rendered text unchanged).
    fn format_type_argument(
        &self,
        arg: &crate::resources::diagnostic_contract::TypeArg,
        locale: Option<&Locale>,
    ) -> String {
        format_placeholder_text(self, locale, &arg.text)
    }

    /// Java: protected abstract String formatDiagnostic(JCDiagnostic d, Locale
    /// locale)
    fn format_diagnostic_impl(&self, d: &JCDiagnostic, locale: Option<&Locale>) -> String;

    /// Java: the abstract formatMessage(JCDiagnostic, Locale) virtual call used
    /// by formatArgument / formatSubdiagnostic (implemented by Basic/Raw).
    fn format_message_impl(&self, d: &JCDiagnostic, locale: Option<&Locale>) -> String;

    /// Java: public String formatKind(JCDiagnostic d, Locale l)
    fn format_kind_impl(&self, d: &JCDiagnostic, locale: Option<&Locale>) -> String {
        match d.get_type() {
            DiagnosticType::FRAGMENT => String::new(),
            DiagnosticType::NOTE => self.localize(locale, "compiler.note.note", &[]),
            DiagnosticType::WARNING => self.localize(locale, "compiler.warn.warning", &[]),
            DiagnosticType::ERROR => self.localize(locale, "compiler.err.error", &[]),
        }
    }

    /// Java: public String format(JCDiagnostic d, Locale locale)
    fn format_impl(&self, d: &JCDiagnostic, locale: Option<&Locale>) -> String {
        // Java: allCaptured = List.nil(); (the captured-type Printer is not
        // ported yet, REGISTERED).
        self.format_diagnostic_impl(d, locale)
    }

    /// Java: public String formatPosition(JCDiagnostic d, PositionKind pk,
    /// Locale l)
    fn format_position_impl(
        &self,
        d: &JCDiagnostic,
        pk: PositionKind,
        locale: Option<&Locale>,
    ) -> String {
        let _ = locale;
        assert_ne!(
            d.get_position(),
            NOPOS as i64,
            "AssertionError: position is NOPOS"
        );
        self.get_position(d, pk).to_string()
    }

    /// Java: private long getPosition(JCDiagnostic d, PositionKind pk)
    fn get_position(&self, d: &JCDiagnostic, pk: PositionKind) -> i64 {
        match pk {
            PositionKind::Start => d.get_int_start_position() as i64,
            PositionKind::End => d.get_int_end_position() as i64,
            PositionKind::Line => d.get_line_number(),
            PositionKind::Column => d.get_column_number(),
            PositionKind::Offset => d.get_int_position() as i64,
        }
    }

    /// Java: public String formatSource(JCDiagnostic d, boolean fullname,
    /// Locale l)
    fn format_source_impl(
        &self,
        d: &JCDiagnostic,
        fullname: bool,
        locale: Option<&Locale>,
    ) -> String {
        let _ = locale;
        let Some(file_object) = d.get_source() else {
            // Java: throw new IllegalArgumentException(); // d should have source set
            panic!("IllegalArgumentException: diagnostic has no source");
        };
        if fullname {
            return file_object.get_name().to_string_lossy();
        }
        // Java: PathFileObject.getShortName() / PathFileObject.getSimpleName(fo)
        // both return the last path component of the file object name. The port
        // has no trait-object downcast, so the last component of getName() is
        // used (registered adapter; the values are identical for the ported file
        // objects).
        let name = file_object.get_name().to_string_lossy();
        name.rsplit('/')
            .next()
            .map(|part| part.to_string())
            .unwrap_or(name)
    }

    /// Format the arguments of a given diagnostic.
    ///
    /// Java: protected Collection<String> formatArguments(JCDiagnostic d,
    /// Locale l)
    fn format_arguments(&self, d: &JCDiagnostic, locale: Option<&Locale>) -> Vec<String> {
        d.get_args()
            .iter()
            .map(|arg| self.format_argument(d, arg, locale))
            .collect()
    }

    /// Format a single argument of a given diagnostic.
    ///
    /// Java: protected String formatArgument(JCDiagnostic d, Object arg,
    /// Locale l)
    fn format_argument(
        &self,
        d: &JCDiagnostic,
        arg: &DiagnosticArg,
        locale: Option<&Locale>,
    ) -> String {
        format_argument_default(self, d, arg, locale)
    }

    /// Format an iterable argument of a given diagnostic.
    ///
    /// Java: protected String formatIterable(JCDiagnostic d, Iterable<?> it,
    /// Locale l)
    fn format_iterable(
        &self,
        d: &JCDiagnostic,
        items: &[DiagnosticArg],
        locale: Option<&Locale>,
    ) -> String {
        let mut result = String::new();
        let mut sep = "";
        // Java: UndetVar.addBound prepends equality bounds, so the rendered
        // incompatible.eq.bounds list is in reverse insertion order; the
        // ported bounds list keeps insertion order.
        let reverse = d.get_code_string().ends_with("incompatible.eq.bounds");
        let count = items.len();
        for index in 0..count {
            let item = if reverse {
                &items[count - 1 - index]
            } else {
                &items[index]
            };
            result.push_str(sep);
            result.push_str(&self.format_argument(d, item, locale));
            sep = ",";
        }
        result
    }

    /// Format all the subdiagnostics attached to a given diagnostic.
    ///
    /// Java: protected List<String> formatSubdiagnostics(JCDiagnostic d,
    /// Locale l)
    fn format_subdiagnostics(&self, d: &JCDiagnostic, locale: Option<&Locale>) -> Vec<String> {
        let mut subdiagnostics = Vec::new();
        let max_depth = self.config_multiline_limit(MultilineLimit::Depth);
        if max_depth == -1 || self.depth() < max_depth {
            let depth = self.depth();
            self.set_depth(depth + 1);
            let max_count = self.config_multiline_limit(MultilineLimit::Length);
            let mut count = 0;
            for sub in d.get_subdiagnostics().iter() {
                if max_count == -1 || count < max_count {
                    subdiagnostics.push(self.format_subdiagnostic(d, &sub, locale));
                    count += 1;
                } else {
                    break;
                }
            }
            self.set_depth(depth);
        }
        subdiagnostics
    }

    /// Format a subdiagnostic attached to a given diagnostic.
    ///
    /// Java: protected String formatSubdiagnostic(JCDiagnostic parent,
    /// JCDiagnostic sub, Locale l)
    fn format_subdiagnostic(
        &self,
        parent: &JCDiagnostic,
        sub: &JCDiagnostic,
        locale: Option<&Locale>,
    ) -> String {
        let _ = parent;
        // Java: formatSubdiagnostic formats the attached diagnostic as a nested
        // message; keep the depth incremented so rich-only where clauses are
        // appended by the outermost format() call, not to the nested detail.
        let depth = self.depth();
        self.set_depth(depth + 1);
        let text = self.format_message_impl(sub, locale);
        self.set_depth(depth);
        text
    }

    /// Format the faulty source code line and point to the error.
    ///
    /// Java: protected String formatSourceLine(JCDiagnostic d, int nSpaces)
    fn format_source_line(&self, d: &JCDiagnostic, n_spaces: i32) -> String {
        let source = d.get_diagnostic_source();
        let pos = d.get_int_position();
        assert_ne!(pos, NOPOS, "AssertionError: diagnostic has no position");
        let line = match &source {
            None => None,
            Some(source) => source.get_line(pos),
        };
        let Some(line) = line else {
            return String::new();
        };
        let mut result = self.indent(&line.to_string_lossy(), n_spaces);
        let source = source.expect("source is present when line is present");
        let col = source.get_column_number(pos, false);
        if self.config_caret_enabled() {
            result.push('\n');
            for i in 0..col - 1 {
                let ch = line.char_at(i as usize);
                result.push(if ch == '\t' as u16 { '\t' } else { ' ' });
            }
            result.push_str(&self.indent("^", n_spaces));
        }
        result
    }

    /// Java: protected String formatLintCategory(JCDiagnostic d, Locale l)
    fn format_lint_category(&self, d: &JCDiagnostic, locale: Option<&Locale>) -> String {
        match d.get_lint_category() {
            None => String::new(),
            Some(lc) => self.localize(
                locale,
                "compiler.warn.lintOption",
                &[lc.option().to_string()],
            ),
        }
    }

    /// Converts a String into a locale-dependent representation accordingly to
    /// a given locale.
    ///
    /// Java: protected String localize(Locale l, String key, Object... args)
    fn localize(&self, locale: Option<&Locale>, key: &str, args: &[String]) -> String {
        // Java: the upstream messages field is never null in this branch (the raw
        // formatter overrides localize); a null messages object would throw a
        // NullPointerException at the localize call.
        let Some(messages) = self.messages() else {
            panic!("NullPointerException: localize without JavacMessages");
        };
        messages.get_localized_string_for(locale, key, args)
    }

    /// Java: public boolean displaySource(JCDiagnostic d)
    fn display_source_impl(&self, d: &JCDiagnostic) -> bool {
        self.config_visible().contains(&DiagnosticPart::Source)
            && d.get_type() != DiagnosticType::FRAGMENT
            && d.get_int_position() != NOPOS
    }

    /// Java: public boolean isRaw()
    fn is_raw_impl(&self) -> bool {
        false
    }

    /// Creates a string with a given amount of empty spaces.
    ///
    /// Java: protected String indentString(int nSpaces)
    fn indent_string(&self, n_spaces: i32) -> String {
        let spaces = "                        ";
        if n_spaces as usize <= spaces.len() {
            spaces[..n_spaces as usize].to_string()
        } else {
            let mut result = String::new();
            for _ in 0..n_spaces {
                result.push(' ');
            }
            result
        }
    }

    /// Indent a string by prepending a given amount of empty spaces to each
    /// line of the string.
    ///
    /// Java: protected String indent(String s, int nSpaces)
    fn indent(&self, s: &str, n_spaces: i32) -> String {
        let indent = self.indent_string(n_spaces);
        let mut result = String::new();
        let mut nl = "";
        for line in s.split('\n') {
            result.push_str(nl);
            result.push_str(&indent);
            result.push_str(line);
            nl = "\n";
        }
        result
    }
}

/// The body of the default formatArgument implementation.
///
/// PORTING_RULES: Rust cannot call a trait's default method implementation from
/// an override (the qualified trait call dispatches to the override and would
/// recurse); the default body therefore lives in this free function and the
/// trait default / the Raw override both call it (registered section 9).
pub fn format_argument_default<F: AbstractDiagnosticFormatter + ?Sized>(
    formatter: &F,
    d: &JCDiagnostic,
    arg: &DiagnosticArg,
    locale: Option<&Locale>,
) -> String {
    if let Some(type_arg) = arg.downcast_ref::<crate::resources::diagnostic_contract::TypeArg>() {
        return formatter.format_type_argument(type_arg, locale);
    }
    if let Some(diagnostic) = arg.downcast_ref::<JCDiagnostic>() {
        // Java: String s = formatMessage(diagnostic, l) with depth++ / depth--.
        let depth = formatter.depth();
        formatter.set_depth(depth + 1);
        let s = formatter.format_message_impl(diagnostic, locale);
        formatter.set_depth(depth);
        return s;
    }
    if let Some(items) = arg.downcast_ref::<Vec<DiagnosticArg>>() {
        let items: Vec<DiagnosticArg> = items.clone();
        return formatter.format_iterable(d, &items, locale);
    }
    // NOT PORTED: Type / Symbol (code/Printer), Profile / Option / Target /
    // Source (jvm/code/main batches). The ported branches follow.
    if let Some(file_object) = arg.downcast_ref::<Rc<dyn JavaFileObject>>() {
        return file_object.get_name().to_string_lossy();
    }
    if let Some(formattable) = arg.downcast_ref::<Rc<dyn Formattable>>() {
        // Java: formattable.toString(l, messages) would throw a
        // NullPointerException when messages is null (raw formatter fallback).
        let Some(messages) = formatter.messages() else {
            panic!("NullPointerException: Formattable argument without JavacMessages");
        };
        let locale = locale.cloned().unwrap_or_else(Locale::default_locale);
        return formattable.to_string_with_locale(&locale, messages.as_ref());
    }
    // NOT PORTED before the code/Source; now ported.
    if let Some(source) = arg.downcast_ref::<crate::code::source::Source>() {
        return source.name().to_string();
    }
    if let Some(tag) = arg.downcast_ref::<Tag>() {
        let name = format!("{tag:?}").to_lowercase();
        return formatter.localize(locale, &format!("compiler.misc.tree.tag.{name}"), &[]);
    }
    if let Some(value) = arg.downcast_ref::<JavaString>() {
        return format_placeholder_text(formatter, locale, &value.to_string_lossy());
    }
    if let Some(value) = arg.downcast_ref::<String>() {
        return format_placeholder_text(formatter, locale, value);
    }
    if let Some(value) = arg.downcast_ref::<i32>() {
        return value.to_string();
    }
    if let Some(value) = arg.downcast_ref::<i64>() {
        return value.to_string();
    }
    if let Some(value) = arg.downcast_ref::<bool>() {
        return value.to_string();
    }
    if let Some(value) = arg.downcast_ref::<crate::util::name::Name>() {
        return value.to_java_string().to_string_lossy();
    }
    // Java: Format argument instanceof Option -> option.primaryName.
    if let Some(option) = arg.downcast_ref::<crate::main::option::Option>() {
        return option.primary_name().to_string();
    }
    // Java: Format argument instanceof Target -> target.name.
    if let Some(target) = arg.downcast_ref::<crate::jvm::target::Target>() {
        return target.name().to_string();
    }
    // Diagnostic argument type domain is closed: the constructors only store
    // JavaString/String/JCDiagnostic/JavaFileObject values (see the module
    // header); an unknown value cannot be produced by the ported code.
    crate::util::assert::error_msg("unreachable diagnostic argument type");
}

/// P4 placeholder arguments that carry a resource key instead of a formatted
/// value: KindNameArg currently stores "kindname.*" and reason_place stores the
/// fragment's "compiler.misc.*" key. Localize them at formatting time so both
/// paths use the formatter's locale (the javac KindName/LocalizedString
/// objects render themselves through JavacMessages).
pub(crate) fn format_placeholder_text<F: AbstractDiagnosticFormatter + ?Sized>(
    formatter: &F,
    locale: Option<&Locale>,
    text: &str,
) -> String {
    if let Some(name) = text.strip_prefix("kindname.") {
        return formatter.localize(
            locale,
            &format!("compiler.misc.kindname.{name}"),
            &[],
        );
    }
    if text.starts_with("compiler.misc.") {
        return formatter.localize(locale, text, &[]);
    }
    text.to_string()
}

/// Java: public static class SimpleConfiguration implements Configuration
pub struct SimpleConfiguration {
    multiline_limits: Vec<(MultilineLimit, i32)>,
    visible_parts: Vec<DiagnosticPart>,
    caret_enabled: bool,
}

impl SimpleConfiguration {
    /// Java: public SimpleConfiguration(Set<DiagnosticPart> parts)
    pub fn new(parts: Vec<DiagnosticPart>) -> SimpleConfiguration {
        let mut config = SimpleConfiguration {
            multiline_limits: Vec::new(),
            visible_parts: Vec::new(),
            caret_enabled: false,
        };
        config.set_visible(&parts);
        config.set_multiline_limit(MultilineLimit::Depth, -1);
        config.set_multiline_limit(MultilineLimit::Length, -1);
        config.set_caret_enabled(true);
        config
    }

    /// Java: @SuppressWarnings("fallthrough")
    ///       public SimpleConfiguration(Options options, Set<DiagnosticPart> parts)
    pub fn new_with_options(options: &Options, parts: Vec<DiagnosticPart>) -> SimpleConfiguration {
        let mut config = SimpleConfiguration::new(parts);
        if let Some(show_source) = options.get("diags.showSource") {
            if show_source == "true" {
                config.set_visible_part(DiagnosticPart::Source, true);
            } else if show_source == "false" {
                config.set_visible_part(DiagnosticPart::Source, false);
            }
        }
        if let Some(diag_opts) = options.get("diags.formatterOptions") {
            // override -XDshowSource
            let args: Vec<&str> = diag_opts.split(',').collect();
            if args.contains(&"short") {
                config.set_visible_part(DiagnosticPart::Details, false);
                config.set_visible_part(DiagnosticPart::Subdiagnostics, false);
            }
            if args.contains(&"source") {
                config.set_visible_part(DiagnosticPart::Source, true);
            }
            if args.contains(&"-source") {
                config.set_visible_part(DiagnosticPart::Source, false);
            }
        }
        if let Some(multi_policy) = options.get("diags.multilinePolicy") {
            if multi_policy == "disabled" {
                config.set_visible_part(DiagnosticPart::Subdiagnostics, false);
            } else if let Some(limit_string) = multi_policy.strip_prefix("limit:") {
                let limits: Vec<&str> = limit_string.split(':').collect();
                // Java: switch (limits.length) { case 2: DEPTH; case 1: LENGTH; }
                // with fallthrough and NumberFormatException -> reset both limits.
                let parsed: Result<(), ()> = (|| {
                    match limits.len() {
                        2 => {
                            if limits[1] != "*" {
                                config.set_multiline_limit(
                                    MultilineLimit::Depth,
                                    limits[1].parse::<i32>().map_err(|_| ())?,
                                );
                            }
                            if limits[0] != "*" {
                                config.set_multiline_limit(
                                    MultilineLimit::Length,
                                    limits[0].parse::<i32>().map_err(|_| ())?,
                                );
                            }
                        }
                        1 if limits[0] != "*" => {
                            config.set_multiline_limit(
                                MultilineLimit::Length,
                                limits[0].parse::<i32>().map_err(|_| ())?,
                            );
                        }
                        1 => {}
                        _ => {}
                    }
                    Ok(())
                })();
                if parsed.is_err() {
                    config.set_multiline_limit(MultilineLimit::Depth, -1);
                    config.set_multiline_limit(MultilineLimit::Length, -1);
                }
            }
        }
        let show_caret = options.get("diags.showCaret");
        if show_caret.as_deref() == Some("false") {
            config.set_caret_enabled(false);
        } else {
            config.set_caret_enabled(true);
        }
        config
    }

    /// Java: public void setVisiblePart(DiagnosticPart diagParts, boolean
    /// enabled)
    pub fn set_visible_part(&mut self, diag_part: DiagnosticPart, enabled: bool) {
        if enabled {
            if !self.visible_parts.contains(&diag_part) {
                self.visible_parts.push(diag_part);
            }
        } else {
            self.visible_parts.retain(|part| *part != diag_part);
        }
    }

    /// Java: public void setCaretEnabled(boolean caretEnabled)
    pub fn set_caret_enabled(&mut self, caret_enabled: bool) {
        self.caret_enabled = caret_enabled;
    }

    /// Java: public boolean isCaretEnabled()
    pub fn is_caret_enabled(&self) -> bool {
        self.caret_enabled
    }
}

impl Configuration for SimpleConfiguration {
    fn set_visible(&mut self, visible_parts: &[DiagnosticPart]) {
        self.visible_parts = visible_parts.to_vec();
    }

    fn get_visible(&self) -> Vec<DiagnosticPart> {
        self.visible_parts.clone()
    }

    fn set_multiline_limit(&mut self, limit: MultilineLimit, value: i32) {
        let value = if value < -1 { -1 } else { value };
        if let Some(entry) = self
            .multiline_limits
            .iter_mut()
            .find(|(candidate, _)| *candidate == limit)
        {
            entry.1 = value;
        } else {
            self.multiline_limits.push((limit, value));
        }
    }

    fn get_multiline_limit(&self, limit: MultilineLimit) -> i32 {
        self.multiline_limits
            .iter()
            .find(|(candidate, _)| *candidate == limit)
            .map(|(_, value)| *value)
            .unwrap_or(-1)
    }
}
