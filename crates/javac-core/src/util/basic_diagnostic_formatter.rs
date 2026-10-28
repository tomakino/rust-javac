// Origin: com/sun/tools/javac/util/BasicDiagnosticFormatter.java @ 982d53c (jdk-17.0.19-ga)

//! A basic formatter for diagnostic messages.
//! The basic formatter will format a diagnostic according to one of three
//! format patterns, depending on whether or not the source name and position
//! are set. The formatter supports a printf-like string for patterns with the
//! following special characters:
//! <ul>
//! <li>%b: the base of the source name
//! <li>%f: the source name (full absolute path)
//! <li>%l: the line number of the diagnostic, derived from the character offset
//! <li>%c: the column number of the diagnostic, derived from the character offset
//! <li>%o: the character offset of the diagnostic if set
//! <li>%p: the prefix for the diagnostic, derived from the diagnostic type
//! <li>%t: the prefix as it normally appears in standard diagnostics. In this
//!        case, no prefix is shown if the type is ERROR and if a source name is
//!        set
//! <li>%m: the text or the diagnostic, including any appropriate arguments
//! <li>%_: space delimiter, useful for formatting purposes
//! </ul>
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES:
//! - the Java subclassing (BasicDiagnosticFormatter extends
//!   AbstractDiagnosticFormatter) maps to implementing the
//!   AbstractDiagnosticFormatter trait; the formatter state (messages /
//!   configuration / depth) lives in this struct;
//! - the name clash with the DiagnosticFormatter trait methods is resolved by
//!   the _impl-suffixed trait methods (see abstract_diagnostic_formatter.rs);
//!   the DiagnosticFormatter implementation forwards to them;
//! - BasicConfiguration(Options) needs the not-yet-ported Options and is not
//!   provided (TODO: the diags.* option decoding is registered in the Options
//!   ); BasicConfiguration::new() is the ported no-options constructor,
//!   including the upstream behaviour that sourcePosition stays null (None)
//!   until set.

use std::cell::Cell;
use std::rc::Rc;

use crate::api::diagnostic_formatter::{
    Configuration, DiagnosticFormatter, DiagnosticPart, MultilineLimit, PositionKind,
};
use crate::util::abstract_diagnostic_formatter::{AbstractDiagnosticFormatter, SimpleConfiguration};
use crate::util::javac_messages::{JavacMessages, Locale};
use crate::util::options::Options;
use crate::util::jc_diagnostic::JCDiagnostic;
use crate::util::layout_characters::{DIAG_INC, DETAILS_INC};
use crate::util::position::NOPOS;

/// A basic formatter for diagnostic messages.
///
/// Java: public class BasicDiagnosticFormatter extends
/// AbstractDiagnosticFormatter
pub struct BasicDiagnosticFormatter {
    messages: Rc<JavacMessages>,
    config: BasicConfiguration,
    depth: Cell<i32>,
}

impl BasicDiagnosticFormatter {
    /// Create a standard basic formatter.
    ///
    /// Java: public BasicDiagnosticFormatter(JavacMessages msgs)
    pub fn new(messages: Rc<JavacMessages>) -> BasicDiagnosticFormatter {
        BasicDiagnosticFormatter {
            messages,
            config: BasicConfiguration::new(),
            depth: Cell::new(0),
        }
    }

    /// Java: public BasicDiagnosticFormatter(Options options, JavacMessages msgs)
    pub fn new_with_options(messages: Rc<JavacMessages>, options: &Options) -> BasicDiagnosticFormatter {
        BasicDiagnosticFormatter {
            messages,
            config: BasicConfiguration::new_with_options(options),
            depth: Cell::new(0),
        }
    }

    /// Java: public BasicConfiguration getConfiguration()
    pub fn get_configuration(&self) -> &BasicConfiguration {
        &self.config
    }

    /// Java: public BasicConfiguration getConfiguration() (mutable access used
    /// by the configuration setters; Rust cannot return the trait view here).
    pub fn get_configuration_mut(&mut self) -> &mut BasicConfiguration {
        &mut self.config
    }

    /// Java: public String formatDiagnostic(JCDiagnostic d, Locale l)
    fn format_diagnostic_basic(&self, d: &JCDiagnostic, locale: Option<&Locale>) -> String {
        let locale = match locale {
            // Java: if (l == null) l = messages.getCurrentLocale();
            None => self.messages.get_current_locale().clone(),
            Some(locale) => locale.clone(),
        };
        let format = self.select_format(d);
        let mut buf = String::new();
        let chars: Vec<char> = format.chars().collect();
        let mut i = 0usize;
        while i < chars.len() {
            let mut c = chars[i];
            let mut meta = false;
            if c == '%' && i < chars.len() - 1 {
                meta = true;
                i += 1;
                c = chars[i];
            }
            if meta {
                buf.push_str(&self.format_meta(c, d, &locale));
            } else {
                buf.push(c);
            }
            i += 1;
        }
        if self.depth.get() == 0 {
            self.add_source_line_if_needed(d, buf)
        } else {
            buf
        }
    }

    /// Java: public String formatMessage(JCDiagnostic d, Locale l)
    fn format_message_basic(&self, d: &JCDiagnostic, locale: Option<&Locale>) -> String {
        let mut current_indentation = 0i32;
        let mut buf = String::new();
        let args = self.format_arguments(d, locale);
        let msg = self.localize(locale, &d.get_code_string(), &args);
        let mut lines = java_split_lines(&msg);
        if lines.is_empty() {
            // will happen when msg only contains one or more separators:
            // "\n", "\n\n", etc.
            lines = vec![String::new()];
        }
        let visible = self.config_visible();
        if visible.contains(&DiagnosticPart::Summary) {
            current_indentation += self.config.get_indentation(DiagnosticPart::Summary);
            buf.push_str(&self.indent(&lines[0], current_indentation)); // summary
        }
        if lines.len() > 1 && visible.contains(&DiagnosticPart::Details) {
            current_indentation += self.config.get_indentation(DiagnosticPart::Details);
            for line in lines.iter().skip(1) {
                buf.push('\n');
                buf.push_str(&self.indent(line, current_indentation));
            }
        }
        if d.is_multiline() && visible.contains(&DiagnosticPart::Subdiagnostics) {
            current_indentation += self
                .config
                .get_indentation(DiagnosticPart::Subdiagnostics);
            for sub in self.format_subdiagnostics(d, locale) {
                buf.push('\n');
                buf.push_str(&self.indent(&sub, current_indentation));
            }
        }
        buf
    }

    /// Java: protected String addSourceLineIfNeeded(JCDiagnostic d, String msg)
    fn add_source_line_if_needed(&self, d: &JCDiagnostic, msg: String) -> String {
        if !self.display_source_impl(d) {
            return msg;
        }
        let indent_source = self.config.get_indentation(DiagnosticPart::Source);
        let source_line = format!("\n{}", self.format_source_line(d, indent_source));
        let single_line = !msg.contains('\n');
        if single_line || self.config.get_source_position() == Some(SourcePosition::BOTTOM) {
            format!("{msg}{source_line}")
        } else {
            // Java: msg.replaceFirst("\n", Matcher.quoteReplacement(sourceLine) + "\n")
            let mut result = String::new();
            match msg.split_once('\n') {
                Some((head, tail)) => {
                    result.push_str(head);
                    result.push_str(&source_line);
                    result.push('\n');
                    result.push_str(tail);
                }
                None => result.push_str(&msg),
            }
            result
        }
    }

    /// Java: protected String formatMeta(char c, JCDiagnostic d, Locale l)
    fn format_meta(&self, c: char, d: &JCDiagnostic, locale: &Locale) -> String {
        match c {
            'b' => self.format_source_impl(d, false, Some(locale)),
            'e' => self.format_position_impl(d, PositionKind::End, Some(locale)),
            'f' => self.format_source_impl(d, true, Some(locale)),
            'l' => self.format_position_impl(d, PositionKind::Line, Some(locale)),
            'c' => self.format_position_impl(d, PositionKind::Column, Some(locale)),
            'o' => self.format_position_impl(d, PositionKind::Offset, Some(locale)),
            'p' => self.format_kind_impl(d, Some(locale)),
            's' => self.format_position_impl(d, PositionKind::Start, Some(locale)),
            't' => {
                let use_prefix = match d.get_type() {
                    crate::util::jc_diagnostic::DiagnosticType::FRAGMENT => false,
                    crate::util::jc_diagnostic::DiagnosticType::ERROR => {
                        d.get_int_position() == NOPOS
                    }
                    _ => true,
                };
                if use_prefix {
                    self.format_kind_impl(d, Some(locale))
                } else {
                    String::new()
                }
            }
            'm' => self.format_message_impl(d, Some(locale)),
            'L' => self.format_lint_category(d, Some(locale)),
            '_' => " ".to_string(),
            '%' => "%".to_string(),
            _ => c.to_string(),
        }
    }

    /// Java: private String selectFormat(JCDiagnostic d)
    fn select_format(&self, d: &JCDiagnostic) -> String {
        let source = d.get_diagnostic_source();
        let mut format = self
            .config
            .get_format(BasicFormatKind::DEFAULT_NO_POS_FORMAT);
        if let Some(source) = source {
            // Java: source != DiagnosticSource.NO_SOURCE; the port's NO_SOURCE
            // has no file, so a file-less source selects the no-position format.
            if source.get_file().is_some() && d.get_int_position() != NOPOS {
                format = self.config.get_format(BasicFormatKind::DEFAULT_POS_FORMAT);
            } else if let Some(file) = source.get_file()
                && file.get_kind()
                    == javac_tools::tools::java_file_object::Kind::CLASS
            {
                format = self.config.get_format(BasicFormatKind::DEFAULT_CLASS_FORMAT);
            }
        }
        format
    }
}

impl AbstractDiagnosticFormatter for BasicDiagnosticFormatter {
    fn messages(&self) -> Option<&Rc<JavacMessages>> {
        Some(&self.messages)
    }

    fn depth(&self) -> i32 {
        self.depth.get()
    }

    fn set_depth(&self, depth: i32) {
        self.depth.set(depth);
    }

    fn config_visible(&self) -> Vec<DiagnosticPart> {
        self.config.get_visible()
    }

    fn config_multiline_limit(&self, limit: MultilineLimit) -> i32 {
        self.config.get_multiline_limit(limit)
    }

    fn config_caret_enabled(&self) -> bool {
        self.config.is_caret_enabled()
    }

    fn format_diagnostic_impl(&self, d: &JCDiagnostic, locale: Option<&Locale>) -> String {
        self.format_diagnostic_basic(d, locale)
    }

    fn format_message_impl(&self, d: &JCDiagnostic, locale: Option<&Locale>) -> String {
        self.format_message_basic(d, locale)
    }
}

impl DiagnosticFormatter<JCDiagnostic> for BasicDiagnosticFormatter {
    fn display_source(&self, diag: &JCDiagnostic) -> bool {
        self.display_source_impl(diag)
    }

    fn format(&self, diag: &JCDiagnostic, locale: Option<&Locale>) -> String {
        self.format_impl(diag, locale)
    }

    fn format_message(&self, diag: &JCDiagnostic, locale: &Locale) -> String {
        self.format_message_impl(diag, Some(locale))
    }

    fn format_kind(&self, diag: &JCDiagnostic, locale: &Locale) -> String {
        self.format_kind_impl(diag, Some(locale))
    }

    fn format_source(&self, diag: &JCDiagnostic, fullname: bool, locale: &Locale) -> String {
        self.format_source_impl(diag, fullname, Some(locale))
    }

    fn format_position(
        &self,
        diag: &JCDiagnostic,
        pk: PositionKind,
        locale: &Locale,
    ) -> String {
        self.format_position_impl(diag, pk, Some(locale))
    }

    fn get_configuration(&mut self) -> &mut dyn Configuration {
        &mut self.config
    }
}

/// Java: static String[] split lines like String.split("\n") (trailing empty
/// strings are removed, which is the Java split behaviour used upstream).
fn java_split_lines(msg: &str) -> Vec<String> {
    let mut lines: Vec<String> = msg.split('\n').map(String::from).collect();
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines
}

/// Java: public static class BasicConfiguration extends SimpleConfiguration
pub struct BasicConfiguration {
    simple: SimpleConfiguration,
    indentation_levels: Vec<(DiagnosticPart, i32)>,
    available_formats: Vec<(BasicFormatKind, String)>,
    source_position: Option<SourcePosition>,
}

impl BasicConfiguration {
    /// Java: public BasicConfiguration()
    pub fn new() -> BasicConfiguration {
        let mut config = BasicConfiguration {
            simple: SimpleConfiguration::new(vec![
                DiagnosticPart::Summary,
                DiagnosticPart::Details,
                DiagnosticPart::Subdiagnostics,
                DiagnosticPart::Source,
            ]),
            indentation_levels: Vec::new(),
            available_formats: Vec::new(),
            // Java: BasicConfiguration() does not set sourcePosition (stays
            // null) until the Options constructor sets it.
            source_position: None,
        };
        config.init_format();
        config.init_indentation();
        config
    }

    /// Java: @SuppressWarnings("fallthrough")
    ///       public BasicConfiguration(Options options)
    pub fn new_with_options(options: &Options) -> BasicConfiguration {
        let mut config = BasicConfiguration {
            simple: SimpleConfiguration::new_with_options(
                options,
                vec![
                    DiagnosticPart::Summary,
                    DiagnosticPart::Details,
                    DiagnosticPart::Subdiagnostics,
                    DiagnosticPart::Source,
                ],
            ),
            indentation_levels: Vec::new(),
            available_formats: Vec::new(),
            source_position: None,
        };
        config.init_format();
        config.init_indentation();
        if options.is_set("diags.legacy") {
            config.init_old_format();
        }
        if let Some(fmt) = options.get("diags.layout") {
            if fmt == "OLD" {
                config.init_old_format();
            } else {
                config.init_formats_from(&fmt);
            }
        }
        let src_pos = options.get("diags.sourcePosition");
        if src_pos.as_deref() == Some("bottom") {
            config.set_source_position(SourcePosition::BOTTOM);
        } else {
            config.set_source_position(SourcePosition::AFTER_SUMMARY);
        }
        if let Some(indent) = options.get("diags.indent") {
            let levels: Vec<&str> = indent.split('|').collect();
            let parsed: Result<(), ()> = (|| {
                match levels.len() {
                    5 => {
                        config.set_indentation(
                            DiagnosticPart::Jls,
                            levels[4].parse::<i32>().map_err(|_| ())?,
                        );
                        config.set_indentation(
                            DiagnosticPart::Subdiagnostics,
                            levels[3].parse::<i32>().map_err(|_| ())?,
                        );
                        config.set_indentation(
                            DiagnosticPart::Source,
                            levels[2].parse::<i32>().map_err(|_| ())?,
                        );
                        config.set_indentation(
                            DiagnosticPart::Details,
                            levels[1].parse::<i32>().map_err(|_| ())?,
                        );
                        config.set_indentation(
                            DiagnosticPart::Summary,
                            levels[0].parse::<i32>().map_err(|_| ())?,
                        );
                    }
                    4 => {
                        config.set_indentation(
                            DiagnosticPart::Subdiagnostics,
                            levels[3].parse::<i32>().map_err(|_| ())?,
                        );
                        config.set_indentation(
                            DiagnosticPart::Source,
                            levels[2].parse::<i32>().map_err(|_| ())?,
                        );
                        config.set_indentation(
                            DiagnosticPart::Details,
                            levels[1].parse::<i32>().map_err(|_| ())?,
                        );
                        config.set_indentation(
                            DiagnosticPart::Summary,
                            levels[0].parse::<i32>().map_err(|_| ())?,
                        );
                    }
                    3 => {
                        config.set_indentation(
                            DiagnosticPart::Source,
                            levels[2].parse::<i32>().map_err(|_| ())?,
                        );
                        config.set_indentation(
                            DiagnosticPart::Details,
                            levels[1].parse::<i32>().map_err(|_| ())?,
                        );
                        config.set_indentation(
                            DiagnosticPart::Summary,
                            levels[0].parse::<i32>().map_err(|_| ())?,
                        );
                    }
                    2 => {
                        config.set_indentation(
                            DiagnosticPart::Details,
                            levels[1].parse::<i32>().map_err(|_| ())?,
                        );
                        config.set_indentation(
                            DiagnosticPart::Summary,
                            levels[0].parse::<i32>().map_err(|_| ())?,
                        );
                    }
                    _ => {
                        config.set_indentation(
                            DiagnosticPart::Summary,
                            levels[0].parse::<i32>().map_err(|_| ())?,
                        );
                    }
                }
                Ok(())
            })();
            if parsed.is_err() {
                config.init_indentation();
            }
        }
        config
    }

    fn init_format(&mut self) {
        self.init_formats("%f:%l:%_%p%L%m", "%p%L%m", "%f:%_%p%L%m");
    }

    /// Java: private void initOldFormat()
    pub fn init_old_format(&mut self) {
        self.init_formats("%f:%l:%_%t%L%m", "%p%L%m", "%f:%_%t%L%m");
    }

    fn init_formats(&mut self, pos: &str, nopos: &str, clazz: &str) {
        self.available_formats = Vec::new();
        self.set_format(BasicFormatKind::DEFAULT_POS_FORMAT, pos);
        self.set_format(BasicFormatKind::DEFAULT_NO_POS_FORMAT, nopos);
        self.set_format(BasicFormatKind::DEFAULT_CLASS_FORMAT, clazz);
    }

    /// Java: private void initFormats(String fmt)
    pub fn init_formats_from(&mut self, fmt: &str) {
        let formats: Vec<&str> = fmt.split('|').collect();
        match formats.len() {
            3 => {
                self.set_format(BasicFormatKind::DEFAULT_CLASS_FORMAT, formats[2]);
                self.set_format(BasicFormatKind::DEFAULT_NO_POS_FORMAT, formats[1]);
                self.set_format(BasicFormatKind::DEFAULT_POS_FORMAT, formats[0]);
            }
            2 => {
                self.set_format(BasicFormatKind::DEFAULT_NO_POS_FORMAT, formats[1]);
                self.set_format(BasicFormatKind::DEFAULT_POS_FORMAT, formats[0]);
            }
            _ => {
                self.set_format(BasicFormatKind::DEFAULT_POS_FORMAT, formats[0]);
            }
        }
    }

    fn init_indentation(&mut self) {
        self.indentation_levels = Vec::new();
        self.set_indentation(DiagnosticPart::Summary, 0);
        self.set_indentation(DiagnosticPart::Details, DETAILS_INC);
        self.set_indentation(DiagnosticPart::Subdiagnostics, DIAG_INC);
        self.set_indentation(DiagnosticPart::Source, 0);
    }

    /// Get the amount of spaces for a given indentation kind.
    ///
    /// Java: public int getIndentation(DiagnosticPart diagPart)
    pub fn get_indentation(&self, diag_part: DiagnosticPart) -> i32 {
        self.indentation_levels
            .iter()
            .find(|(part, _)| *part == diag_part)
            .map(|(_, value)| *value)
            .unwrap_or(0)
    }

    /// Set the indentation level for various element of a given diagnostic.
    ///
    /// Java: public void setIndentation(DiagnosticPart diagPart, int nSpaces)
    pub fn set_indentation(&mut self, diag_part: DiagnosticPart, n_spaces: i32) {
        if let Some(entry) = self
            .indentation_levels
            .iter_mut()
            .find(|(part, _)| *part == diag_part)
        {
            entry.1 = n_spaces;
        } else {
            self.indentation_levels.push((diag_part, n_spaces));
        }
    }

    /// Java: public void setSourcePosition(SourcePosition sourcePos)
    pub fn set_source_position(&mut self, source_pos: SourcePosition) {
        self.source_position = Some(source_pos);
    }

    /// Java: public SourcePosition getSourcePosition()
    pub fn get_source_position(&self) -> Option<SourcePosition> {
        self.source_position
    }

    /// Java: public void setFormat(BasicFormatKind kind, String s)
    pub fn set_format(&mut self, kind: BasicFormatKind, s: &str) {
        if let Some(entry) = self
            .available_formats
            .iter_mut()
            .find(|(candidate, _)| *candidate == kind)
        {
            entry.1 = s.to_string();
        } else {
            self.available_formats.push((kind, s.to_string()));
        }
    }

    /// Java: public String getFormat(BasicFormatKind kind)
    pub fn get_format(&self, kind: BasicFormatKind) -> String {
        self.available_formats
            .iter()
            .find(|(candidate, _)| *candidate == kind)
            .map(|(_, format)| format.clone())
            .unwrap_or_default()
    }

    /// Java: public boolean isCaretEnabled()
    pub fn is_caret_enabled(&self) -> bool {
        self.simple.is_caret_enabled()
    }

    /// Java: public void setCaretEnabled(boolean caretEnabled)
    pub fn set_caret_enabled(&mut self, caret_enabled: bool) {
        self.simple.set_caret_enabled(caret_enabled);
    }

    /// Java: SimpleConfiguration.setVisiblePart (used by the Options ctor).
    pub fn set_visible_part(&mut self, diag_part: DiagnosticPart, enabled: bool) {
        self.simple.set_visible_part(diag_part, enabled);
    }
}

impl Default for BasicConfiguration {
    fn default() -> BasicConfiguration {
        BasicConfiguration::new()
    }
}

impl Configuration for BasicConfiguration {
    fn set_visible(&mut self, visible_parts: &[DiagnosticPart]) {
        self.simple.set_visible(visible_parts);
    }

    fn get_visible(&self) -> Vec<DiagnosticPart> {
        self.simple.get_visible()
    }

    fn set_multiline_limit(&mut self, limit: MultilineLimit, value: i32) {
        self.simple.set_multiline_limit(limit, value);
    }

    fn get_multiline_limit(&self, limit: MultilineLimit) -> i32 {
        self.simple.get_multiline_limit(limit)
    }
}

/// A source positioning value controls the position (within a given diagnostic
/// message) in which the source line the diagnostic refers to should be
/// displayed (if applicable).
///
/// Java: public enum BasicConfiguration.SourcePosition
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourcePosition {
    /// Source line is displayed after the diagnostic message.
    BOTTOM,
    /// Source line is displayed after the first line of the diagnostic message.
    AFTER_SUMMARY,
}

/// This enum contains all the kinds of formatting patterns supported by a
/// basic diagnostic formatter.
///
/// Java: public enum BasicConfiguration.BasicFormatKind
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BasicFormatKind {
    /// A format string to be used for diagnostics with a given position.
    DEFAULT_POS_FORMAT,
    /// A format string to be used for diagnostics without a given position.
    DEFAULT_NO_POS_FORMAT,
    /// A format string to be used for diagnostics regarding classfiles.
    DEFAULT_CLASS_FORMAT,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::java_string::JavaString;
    use crate::util::jc_diagnostic::{Factory, LintCategory};
    use crate::util::javac_messages::{
        ResourceBundle, ResourceBundleHelper, ResourceBundleTable, DEFAULT_BUNDLE_NAME,
    };
    use javac_tools::error::Error;
    use javac_tools::tools::file_object::FileObject;
    use javac_tools::tools::java_file_object::{JavaFileObject, Kind};
    use javac_tools::uri::URI;
    use std::io::{Read, Write};

    struct TestBundle(ResourceBundleTable);

    impl ResourceBundleHelper for TestBundle {
        fn get_resource_bundle(&self, _locale: &Locale) -> Option<ResourceBundle> {
            Some(ResourceBundle::new(vec![self.0]))
        }
    }

    static TEST_MESSAGES: &[(&str, &str)] = &[
        // The real values of the compiler resource keys (with the trailing
        // space) are overridden to keep the tests independent of the bundle.
        ("compiler.err.error", "error: "),
        ("compiler.warn.warning", "warning: "),
        ("compiler.note.note", "Note: "),
        ("compiler.err.test", "test error {0}"),
        ("compiler.err.multi", "summary\ndetail one\ndetail two"),
        ("compiler.warn.lintOption", "[{0}] "),
        ("compiler.note.sub", "sub message {0}"),
        ("compiler.warn.deprecation", "deprecated {0}"),
    ];

    fn messages() -> Rc<JavacMessages> {
        let mut messages = JavacMessages::new(DEFAULT_BUNDLE_NAME);
        messages.add_helper(Box::new(TestBundle(TEST_MESSAGES)));
        Rc::new(messages)
    }

    struct TestFileObject {
        name: &'static str,
        content: &'static str,
        kind: Kind,
    }

    impl FileObject for TestFileObject {
        fn to_uri(&self) -> URI {
            URI::create(&JavaString::from(self.name))
        }

        fn get_name(&self) -> JavaString {
            JavaString::from(self.name)
        }

        fn open_input_stream(&self) -> Result<Box<dyn Read>, Error> {
            Err(Error::UnsupportedOperationException)
        }

        fn open_output_stream(&self) -> Result<Box<dyn Write>, Error> {
            Err(Error::UnsupportedOperationException)
        }

        fn get_char_content(
            &self,
            _ignore_encoding_errors: bool,
        ) -> Result<Option<JavaString>, Error> {
            Ok(Some(JavaString::from(self.content)))
        }

        fn open_reader(
            &self,
            _ignore_encoding_errors: bool,
        ) -> Result<Box<dyn javac_tools::java_io::Reader>, Error> {
            Err(Error::UnsupportedOperationException)
        }

        fn open_writer(&self) -> Result<Box<dyn javac_tools::java_io::Writer>, Error> {
            Err(Error::UnsupportedOperationException)
        }

        fn get_last_modified(&self) -> i64 {
            0
        }

        fn delete(&self) -> bool {
            false
        }
    }

    impl JavaFileObject for TestFileObject {
        fn get_kind(&self) -> Kind {
            self.kind
        }

        fn is_name_compatible(&self, _simple_name: &JavaString, kind: Kind) -> bool {
            kind == self.kind
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

    fn factory() -> Factory {
        Factory::new(messages(), "compiler")
    }

    fn source(content: &'static str) -> Rc<crate::util::diagnostic_source::DiagnosticSource> {
        Rc::new(crate::util::diagnostic_source::DiagnosticSource::new(
            Rc::new(TestFileObject {
                name: "Test.java",
                content,
                kind: Kind::SOURCE,
            }),
            None,
        ))
    }

    fn position(pos: i32) -> Option<Rc<dyn crate::util::jc_diagnostic::DiagnosticPosition>> {
        Some(Rc::new(
            crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos),
        ))
    }

    fn formatter() -> BasicDiagnosticFormatter {
        BasicDiagnosticFormatter::new(messages())
    }

    #[test]
    fn formats_no_position_diagnostic() {
        let factory = factory();
        let diagnostic = factory.error(
            None,
            None,
            None,
            factory.error_key("test", vec![Rc::new(JavaString::from("X"))]),
        );
        let text = formatter().format(&diagnostic, None);
        assert_eq!(text, "error: test error X");
    }

    #[test]
    fn formats_position_and_source_line_with_caret() {
        let factory = factory();
        let source = source("public class Test {\n    int x = y;\n}\n");
        // The diagnostic points at "y" (line 2, column 13).
        let diagnostic = factory.error(
            None,
            Some(source.clone()),
            position(32),
            factory.error_key("test", vec![Rc::new(JavaString::from("Y"))]),
        );
        let text = formatter().format(&diagnostic, None);
        assert_eq!(
            text,
            "Test.java:2: error: test error Y\n    int x = y;\n            ^"
        );
    }

    #[test]
    fn formats_multiline_details_and_subdiagnostics_indentation() {
        let factory = factory();
        let sub = factory.create(
            crate::util::jc_diagnostic::DiagnosticType::NOTE,
            None,
            None,
            "sub",
            vec![Rc::new(JavaString::from("S"))],
        );
        // Build a multiline diagnostic whose message has details lines and one
        // subdiagnostic; the subdiagnostic is formatted through
        // formatSubdiagnostic (a note message).
        let multi = factory.create(
            crate::util::jc_diagnostic::DiagnosticType::ERROR,
            None,
            None,
            "multi",
            Vec::new(),
        );
        let multiline =
            crate::util::jc_diagnostic::MultilineDiagnostic::new(&multi, crate::util::list::List::of(sub.clone()));
        let text = formatter().format(multiline.as_diagnostic(), None);
        // summary (indent 0), details lines (DETAILS_INC = 2), subdiagnostics
        // (DIAG_INC = 4).
        // The subdiagnostic lines get the accumulated indentation: SUMMARY 0 +
        // DETAILS_INC 2 + DIAG_INC 4 = 6 spaces.
        assert_eq!(
            text,
            "error: summary\n  detail one\n  detail two\n      sub message S"
        );
        let sub_text = formatter().format_message(&sub, &Locale::default_locale());
        assert_eq!(sub_text, "sub message S");
    }

    #[test]
    fn formats_lint_category() {
        let factory = factory();
        let lint = LintCategory::DEPRECATION;
        let diagnostic = factory.warning(
            Some(lint),
            None,
            None,
            factory.warning_key("deprecation", vec![Rc::new(JavaString::from("X"))]),
        );
        let text = formatter().format(&diagnostic, None);
        assert_eq!(text, "warning: [deprecation] deprecated X");
    }

    #[test]
    fn configuration_defaults_and_setters() {
        let mut config = BasicConfiguration::new();
        assert_eq!(config.get_format(BasicFormatKind::DEFAULT_POS_FORMAT), "%f:%l:%_%p%L%m");
        assert_eq!(config.get_format(BasicFormatKind::DEFAULT_NO_POS_FORMAT), "%p%L%m");
        assert_eq!(config.get_format(BasicFormatKind::DEFAULT_CLASS_FORMAT), "%f:%_%p%L%m");
        assert_eq!(config.get_indentation(DiagnosticPart::Summary), 0);
        assert_eq!(config.get_indentation(DiagnosticPart::Details), DETAILS_INC);
        assert_eq!(config.get_indentation(DiagnosticPart::Subdiagnostics), DIAG_INC);
        assert_eq!(config.get_indentation(DiagnosticPart::Source), 0);
        assert!(config.is_caret_enabled());
        assert_eq!(config.get_source_position(), None);
        config.set_source_position(SourcePosition::BOTTOM);
        assert_eq!(config.get_source_position(), Some(SourcePosition::BOTTOM));
        config.set_indentation(DiagnosticPart::Details, 6);
        assert_eq!(config.get_indentation(DiagnosticPart::Details), 6);
        config.set_visible_part(DiagnosticPart::Source, false);
        assert!(!config.get_visible().contains(&DiagnosticPart::Source));
        config.set_format(BasicFormatKind::DEFAULT_NO_POS_FORMAT, "%m");
        assert_eq!(config.get_format(BasicFormatKind::DEFAULT_NO_POS_FORMAT), "%m");
        config.init_old_format();
        assert_eq!(config.get_format(BasicFormatKind::DEFAULT_POS_FORMAT), "%f:%l:%_%t%L%m");
    }

    #[test]
    fn source_display_and_position_formatting() {
        let factory = factory();
        let source = source("public class Test {\n    int x = y;\n}\n");
        let diagnostic = factory.error(
            None,
            Some(source),
            position(32),
            factory.error_key("test", Vec::new()),
        );
        let formatter = formatter();
        assert!(formatter.display_source(&diagnostic));
        assert_eq!(formatter.format_source(&diagnostic, true, &Locale::default_locale()), "Test.java");
        assert_eq!(formatter.format_position(&diagnostic, PositionKind::Line, &Locale::default_locale()), "2");
        assert_eq!(formatter.format_position(&diagnostic, PositionKind::Column, &Locale::default_locale()), "13");
        assert_eq!(formatter.format_position(&diagnostic, PositionKind::Offset, &Locale::default_locale()), "32");
    }
}
