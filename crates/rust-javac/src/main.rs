//! Command line entry point.
//!
//! Ported from: com/sun/tools/javac/Main.java @ 982d53c
//! and com/sun/tools/javac/main/Main.java @ 982d53c (P6 scope).

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use javac_core::api::diagnostic_formatter::DiagnosticFormatter;
use javac_core::code::source::Source;
use javac_core::comp::resolve::{
    InapplicableMethodExceptionPayload, InferenceExceptionPayload,
};
use javac_core::main::mini_compiler::MiniCompiler;
use javac_core::main::option::ArgKind;
use javac_core::main::option::InvalidValueException;
use javac_core::main::option::Option as JavacOption;
use javac_core::main::option::OptionKind;
use javac_core::main::option_helper::OptionHelper;
use javac_core::util::abstract_log::AbstractLog;
use javac_core::util::log::Log;
use javac_core::util::basic_diagnostic_formatter::BasicDiagnosticFormatter;
use javac_core::util::javac_messages::{
    JavacMessages, Locale, DEFAULT_BUNDLE_NAME, JAVAC_BUNDLE_NAME,
};
use javac_core::util::log::DiagnosticHandler;

const OK: i32 = 0;
const ERROR: i32 = 1;
const CMDERR: i32 = 2;
const SYSERR: i32 = 3;
const ABNORMAL: i32 = 4;

fn main() {
    // javac's InapplicableMethodException / InferenceException use Rust panics
    // as their control-flow transport (the payloads are caught again by
    // catch_inapplicable_method_exception / catch_inference_exception). The
    // default panic hook would still print those internal throws to stderr, so
    // suppress exactly those payloads and keep the normal hook for real panics.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = info.payload();
        if payload.is::<InapplicableMethodExceptionPayload>()
            || payload.is::<InferenceExceptionPayload>()
        {
            return;
        }
        default_hook(info);
    }));
    // The compiler uses deep recursion over types/symbols; the process main
    // thread has a small stack on Windows. Run the driver on a dedicated
    // large-stack thread (javac uses the launcher thread's stack as well).
    let handle = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let args: Vec<String> = std::env::args().skip(1).collect();
            let code = match compile(&args) {
                Ok(code) => code,
                Err(message) => {
                    eprintln!("javac: {message}");
                    CMDERR
                }
            };
            // The driver owns the whole compiler object graph: the arenas, the
            // symbol tables, the parse forest, every Rc. The process exits one
            // instruction later, so unwinding that graph buys nothing - the OS
            // reclaims it in one step - and it cost about a sixth of the wall
            // clock. Every class file is written with File::create + write_all
            // and every diagnostic goes through unbuffered stderr, so no state
            // depends on Drop.
            use std::io::Write;
            let _ = std::io::stdout().flush();
            let _ = std::io::stderr().flush();
            std::process::exit(code);
        })
        .expect("cannot spawn compiler thread");
    // Reached only when the worker panicked instead of exiting.
    let code = match handle.join() {
        Ok(()) => ABNORMAL,
        Err(_) => {
            eprintln!("javac: internal error");
            ABNORMAL
        }
    };
    std::process::exit(code);
}

fn info_option(args: &[String]) -> Option<JavacOption> {
    for arg in args {
        if let Some(option) = JavacOption::lookup(arg)
            && matches!(
                option,
                JavacOption::HELP
                    | JavacOption::X
                    | JavacOption::HELP_LINT
                    | JavacOption::VERSION
                    | JavacOption::FULLVERSION
            )
        {
            return Some(option);
        }
    }
    None
}

fn expand_argfiles(args: &[String]) -> Result<Vec<String>, (String, i32)> {
    let mut result = Vec::new();
    let mut queue: VecDeque<String> = args.iter().cloned().collect();
    while let Some(arg) = queue.pop_front() {
        if let Some(path) = arg.strip_prefix('@') {
            // Java: Main/Arguments: an unreadable @file is a launcher error
            // reported as "file not found" with ExitCode.SYSERR
            // (Arguments.java processArgs; Main.java compile).
            let text = std::fs::read_to_string(path)
                .map_err(|_| (format!("file not found: {path}"), SYSERR))?;
            for line in text.lines() {
                let line = line.split('#').next().unwrap_or("").trim();
                if line.is_empty() {
                    continue;
                }
                for word in line.split_whitespace() {
                    queue.push_back(word.to_string());
                }
            }
        } else {
            result.push(arg);
        }
    }
    Ok(result)
}

/// Java: a module name is a dot-separated sequence of Java identifiers.
fn is_valid_module_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    name.split('.').all(|part| {
        let mut chars = part.chars();
        match chars.next() {
            Some(first) => {
                (first.is_alphabetic() || first == '_' || first == '$')
                    && chars.all(|c| c.is_alphanumeric() || c == '_' || c == '$')
            }
            None => false,
        }
    })
}

fn collect_sources(path: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    if path.is_dir() {
        let entries = std::fs::read_dir(path).map_err(|e| format!("cannot read {path:?}: {e}"))?;
        let mut children: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
        children.sort();
        for child in children {
            collect_sources(&child, out)?;
        }
    } else if path.extension().is_some_and(|ext| ext == "java") {
        out.push(path.to_path_buf());
    } else {
        return Err(format!("invalid flag: {}", path.display()));
    }
    Ok(())
}

/// Java: javac's command-line OptionHelper (the Option.process put/get target;
/// Main.CommandLineHelper equivalent). It records the raw option name/value
/// pairs so the Options service can be populated after parsing.
struct CommandLineHelper {
    options: Vec<(String, String)>,
    files: Vec<PathBuf>,
    class_names: Vec<String>,
    log: Option<Rc<Log>>,
}

impl CommandLineHelper {
    fn new(log: Option<Rc<Log>>) -> CommandLineHelper {
        CommandLineHelper {
            options: Vec::new(),
            files: Vec::new(),
            class_names: Vec::new(),
            log,
        }
    }
}

impl OptionHelper for CommandLineHelper {
    fn get(&self, option: JavacOption) -> Option<String> {
        let key = option.primary_name();
        self.options
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.clone())
    }

    fn put(&mut self, name: &str, value: &str) {
        if let Some(entry) = self.options.iter_mut().find(|(key, _)| key == name) {
            entry.1 = value.to_string();
        } else {
            self.options.push((name.to_string(), value.to_string()));
        }
    }

    fn remove(&mut self, name: &str) {
        self.options.retain(|(key, _)| key != name);
    }

    fn handle_file_manager_option(&mut self, _option: JavacOption, _value: &str) -> bool {
        false
    }

    fn get_log(&self) -> Option<Rc<Log>> {
        self.log.clone()
    }

    fn get_own_name(&self) -> String {
        "javac".to_string()
    }

    fn add_file(&mut self, path: &javac_fs::Path) {
        self.files.push(PathBuf::from(path.to_java_string()));
    }

    fn add_class_name(&mut self, name: &str) {
        self.class_names.push(name.to_string());
    }

}

fn select_locale(args: &[String]) -> Locale {
    let mut language: Option<String> = None;
    let mut country: Option<String> = None;
    for arg in args {
        if let Some(property) = arg.strip_prefix("-J-D")
            && let Some((key, value)) = property.split_once('=')
        {
            match key {
                "user.language" => language = Some(value.to_string()),
                "user.country" => country = Some(value.to_string()),
                _ => {}
            }
        }
    }
    if let Some(language) = language {
        let tag = match country {
            Some(country) => format!("{language}_{country}"),
            None => language,
        };
        return Locale::new(tag);
    }
    Locale::default_locale()
}

/// One row of the javac help table (Option.help / helpSynopsis data).
struct HelpEntry {
    names: Vec<&'static str>,
    args_name_key: Option<&'static str>,
    descr_key: &'static str,
    adjacent: bool,
    choices: Vec<String>,
}

/// Java: Option.helpSynopsis (Option.java:1298-1320).
fn help_synopsis(entry: &HelpEntry, name: &str, messages: &JavacMessages) -> String {
    let mut sb = name.to_string();
    if let Some(args_key) = entry.args_name_key {
        if !name.ends_with('=') && !name.ends_with(':') && !entry.adjacent {
            sb.push(' ');
        }
        sb.push_str(&messages.get_localized_string(&format!("javac.{args_key}"), &[]));
    } else if !entry.choices.is_empty() {
        if !name.ends_with(':') {
            sb.push(' ');
        }
        sb.push('{');
        sb.push_str(&entry.choices.join(","));
        sb.push('}');
    }
    sb
}

/// Java: Option.help(Log, String) (Option.java:1260-1290).
fn render_help_entry(
    entry: &HelpEntry,
    messages: &JavacMessages,
    supported: &str,
) -> Vec<String> {
    let synopses = entry
        .names
        .iter()
        .map(|name| help_synopsis(entry, name, messages))
        .collect::<Vec<_>>()
        .join(", ");
    let descr = messages.get_localized_string(
        &format!("javac.{}", entry.descr_key),
        &[supported.to_string()],
    );
    if synopses.len() < 28
        && !descr.contains('\n')
        && 2 + 28 + 1 + descr.len() <= 80
    {
        return vec![format!("  {synopses:<28} {descr}")];
    }
    let mut out = Vec::new();
    if synopses.len() <= 80 {
        out.push(format!("  {synopses}"));
    } else {
        for name in &entry.names {
            out.push(format!("  {}", help_synopsis(entry, name, messages)));
        }
    }
    out.push(format!("        {}", descr.replace('\n', "\n        ")));
    out
}

/// Render the option table exactly like Option.showHelp: sort by primary name,
/// then each Option.help(row).
/// Java: Option.HELP_LINT.process (Option.java:485-508) prints the supported
/// -Xlint keys, one per line, with their descriptions, to standard output.
fn help_lint_text(messages: &JavacMessages) -> String {
    // Java: LINT_KEY_FORMAT = SMALL_INDENT + SMALL_INDENT + "%-" +
    // (DEFAULT_SYNOPSIS_WIDTH - LARGE_INDENT.length()) + "s %s"; the observed
    // javac output pads the key to 21 columns after four spaces.
    let mut out = String::new();
    out.push_str(&messages.get_localized_string("javac.opt.help.lint.header", &[]));
    out.push('\n');
    let row = |key: &str, descr: &str| format!("    {key:<20} {descr}\n");
    out.push_str(&row(
        "all",
        &messages.get_localized_string("javac.opt.Xlint.all", &[]),
    ));
    for category in javac_core::code::lint::LintCategory::values() {
        let key = category.option();
        let descr = messages.get_localized_string(&format!("javac.opt.Xlint.desc.{key}"), &[]);
        out.push_str(&row(key, &descr));
    }
    out.push_str(&row(
        "none",
        &messages.get_localized_string("javac.opt.Xlint.none", &[]),
    ));
    out
}

fn help_text(messages: &JavacMessages, extra: bool) -> String {
    let usage = messages.get_localized_string("javac.msg.usage.header", &["javac".to_string()]);
    // Java: Option.getHelp substitutes the supported source/target releases
    // into "{0}" (jdk.compiler Source.getSupportedVersions).
    let supported = Source::values()
        .iter()
        .filter(|source| source.is_supported())
        .map(|source| source.name())
        .collect::<Vec<_>>()
        .join(", ");
    let kind = if extra {
        OptionKind::EXTENDED
    } else {
        OptionKind::STANDARD
    };
    let mut entries: Vec<HelpEntry> = JavacOption::values()
        .iter()
        .filter_map(|option| {
            if option.get_kind() != kind {
                return None;
            }
            let descr_key = option.descr_key()?;
            Some(HelpEntry {
                names: option.names().to_vec(),
                args_name_key: option.args_name_key(),
                descr_key,
                adjacent: matches!(option.get_arg_kind(), ArgKind::ADJACENT),
                choices: option.choices().unwrap_or_default(),
            })
        })
        .collect();
    // Java: -J is consumed by the launcher and never reaches the Option
    // parser, but Option.J still documents it in the standard help table.
    if !extra {
        entries.push(HelpEntry {
            names: vec!["-J"],
            args_name_key: Some("opt.arg.flag"),
            descr_key: "opt.J",
            adjacent: true,
            choices: Vec::new(),
        });
    }
    entries.sort_by(|a, b| sort_key(a).cmp(&sort_key(b)));
    let mut out = String::new();
    // Java: HELP.process prints msg.usage.header; X (--help-extra) only prints
    // the extended table, a newline and the nonstandard footer.
    if !extra {
        out.push_str(&usage);
        out.push('\n');
    }
    for entry in &entries {
        for line in render_help_entry(entry, messages, &supported) {
            out.push_str(&line);
            out.push('\n');
        }
    }
    // Java: HELP/X.process calls log.printNewline(STDOUT) after showHelp.
    out.push('\n');
    if extra {
        out.push_str(&messages.get_localized_string("javac.msg.usage.nonstandard.footer", &[]));
        out.push('\n');
    }
    out
}

/// Java: showHelp's Collator(US, PRIMARY) ordering. Port adapter: Java's
/// Collator is not available; compare the primary name with the leading '-'
/// removed, case-insensitively (the observed javac order for these names).
fn sort_key(entry: &HelpEntry) -> String {
    entry.names[0]
        .trim_start_matches('-')
        .to_lowercase()
}

/// Emit the command-line warnings javac produces while processing options.
/// Returns the number of warnings written so the final count stays correct.
fn emit_option_warnings(
    args: &[String],
    options: &[(String, String)],
    locale: &Locale,
) -> i32 {
    let messages = JavacMessages::with_locale(DEFAULT_BUNDLE_NAME, Some(locale.clone()));
    let has = |name: &str| options.iter().any(|(key, _)| key == name);
    let options_suppressed = args.iter().any(|arg| {
        if arg == "-nowarn" || arg == "-Xlint:none" {
            return true;
        }
        arg.strip_prefix("-Xlint:")
            .is_some_and(|spec| spec.split(',').any(|part| part == "-options"))
    });
    let mut warnings = 0i32;
    if !options_suppressed
        && let Some((_, spec)) = options.iter().find(|(key, _)| key == "--add-exports")
        && let Some(module) = spec.split('/').next()
        && !module.split('.').all(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| first.is_alphabetic() || first == '_' || first == '$')
                .unwrap_or(false)
                && chars.all(|ch| ch.is_alphanumeric() || ch == '_' || ch == '$')
        })
    {
        eprintln!("warning: bad name in value for --add-exports option: '{module}'");
        warnings += 1;
    }
    if !options_suppressed && has("--add-opens") {
        eprintln!(
            "warning: [options] {}",
            messages.get_localized_string("compiler.warn.addopens.ignored", &[])
        );
        warnings += 1;
    }
    let bootclasspath_set = has("--boot-class-path")
        || args.iter().any(|arg| arg.starts_with("-Xbootclasspath"))
        || has("-extdirs");
    if !options_suppressed && !has("--release") && !has("--system") && !bootclasspath_set {
        if let Some((_, source)) = options.iter().find(|(key, _)| key == "--source") {
            let release = source
                .strip_prefix("1.")
                .unwrap_or(source)
                .parse::<i32>()
                .unwrap_or(17);
            let key = if release < 9 {
                "compiler.warn.source.no.bootclasspath"
            } else if release < 17 {
                "compiler.warn.source.no.system.modules.path"
            } else {
                ""
            };
            if !key.is_empty() {
                eprintln!(
                    "warning: [options] {}",
                    messages.get_localized_string(key, &[release.to_string()])
                );
                warnings += 1;
            }
        }
    }
    let normalize_release = |value: &str| {
        value
            .strip_prefix("1.")
            .unwrap_or(value)
            .parse::<i32>()
            .unwrap_or(17)
    };
    let source_release = options
        .iter()
        .find(|(key, _)| key == "--source" || key == "--release")
        .map(|(_, value)| normalize_release(value));
    let target_release = options
        .iter()
        .find(|(key, _)| key == "--target" || key == "--release")
        .map(|(_, value)| normalize_release(value));
    let mut obsolete = false;
    if !options_suppressed && source_release == Some(7) {
        eprintln!(
            "warning: [options] {}",
            messages.get_localized_string(
                "compiler.warn.option.obsolete.source",
                &[source_release.unwrap_or(7).to_string()],
            )
        );
        warnings += 1;
        obsolete = true;
    }
    if !options_suppressed && target_release == Some(7) {
        eprintln!(
            "warning: [options] {}",
            messages.get_localized_string(
                "compiler.warn.option.obsolete.target",
                &[target_release.unwrap_or(7).to_string()],
            )
        );
        warnings += 1;
        obsolete = true;
    }
    if !options_suppressed && target_release == Some(7) && has("-parameters") {
        eprintln!(
            "warning: -parameters is not supported for target value 7. Use 8 or later."
        );
        warnings += 1;
    }
    if obsolete {
        eprintln!(
            "warning: [options] {}",
            messages.get_localized_string("compiler.warn.option.obsolete.suppression", &[])
        );
        warnings += 1;
    }
    let path_lint = args.iter().any(|arg| {
        arg == "-Xlint:path"
            || arg == "-Xlint:all"
            || arg.starts_with("-Xlint:path,")
            || arg.starts_with("-Xlint:all,")
    }) && !args.iter().any(|arg| arg == "-Xlint:-path");
    if path_lint {
        if let Some((_, extdirs)) = options.iter().find(|(key, _)| key == "-extdirs") {
            for entry in extdirs.split(';').filter(|entry| !entry.is_empty()) {
                if !Path::new(entry).is_dir() {
                    eprintln!(
                        "warning: [path] {}",
                        messages.get_localized_string(
                            "compiler.warn.dir.path.element.not.found",
                            &[entry.to_string()],
                        )
                    );
                    warnings += 1;
                    break;
                }
            }
        }
        if let Some((_, classpath)) = options.iter().find(|(key, _)| key == "--class-path") {
            for entry in classpath.split(';').filter(|entry| !entry.is_empty()) {
                if !Path::new(entry).exists() {
                    eprintln!(
                        "warning: [path] {}",
                        messages.get_localized_string(
                            "compiler.warn.path.element.not.found",
                            &[entry.to_string()],
                        )
                    );
                    warnings += 1;
                    break;
                }
            }
        }
    }
    warnings
}

/// Validate the module options that cannot be combined with --release
/// (javac 17 reports these during option processing).
fn module_option_errors(
    options: &[(String, String)],
    locale: &Locale,
) -> Vec<String> {
    if !options.iter().any(|(key, _)| key == "--release") {
        return Vec::new();
    }
    let messages = JavacMessages::with_locale(DEFAULT_BUNDLE_NAME, Some(locale.clone()));
    let mut errors = Vec::new();
    if let Some((_, spec)) = options.iter().find(|(key, _)| key == "--add-exports") {
        if let Some(module) = spec.split('/').next() {
            if module.starts_with("java.") || module.starts_with("jdk.") {
                errors.push(messages.get_localized_string(
                    "compiler.err.add.exports.with.release",
                    &[module.to_string()],
                ));
            }
        }
    }
    if let Some((_, spec)) = options.iter().find(|(key, _)| key == "--add-reads") {
        if let Some(module) = spec.split('=').next() {
            if module.starts_with("java.") || module.starts_with("jdk.") {
                errors.push(messages.get_localized_string(
                    "compiler.err.add.reads.with.release",
                    &[module.to_string()],
                ));
            }
        }
    }
    errors
}

/// Validate the source/target releases that javac 17 no longer supports. The
/// upstream option processing reports these as errors after the option warnings
/// and does not run the compiler.
fn removed_option_errors(
    options: &[(String, String)],
    locale: &Locale,
) -> Vec<String> {
    let messages = JavacMessages::with_locale(DEFAULT_BUNDLE_NAME, Some(locale.clone()));
    let normalize = |value: &str| {
        value
            .strip_prefix("1.")
            .unwrap_or(value)
            .parse::<i32>()
            .unwrap_or(17)
    };
    let mut errors = Vec::new();
    if let Some((_, source)) = options.iter().find(|(key, _)| key == "--source") {
        let release = normalize(source);
        if release < 7 {
            errors.push(messages.get_localized_string(
                "compiler.err.option.removed.source",
                &[release.to_string(), "7".to_string()],
            ));
        }
    }
    if let Some((_, target)) = options.iter().find(|(key, _)| key == "--target") {
        let release = normalize(target);
        if release < 7 {
            errors.push(messages.get_localized_string(
                "compiler.err.option.removed.target",
                &[release.to_string(), "7".to_string()],
            ));
        }
    }
    errors
}

fn compile(args: &[String]) -> Result<i32, String> {
    let expanded = match expand_argfiles(args) {
        Ok(expanded) => expanded,
        Err((message, code)) => {
            eprintln!("error: {message}");
            return Ok(code);
        }
    };
    let locale = select_locale(&expanded);
    // Apply the locale to compiler diagnostics (Java: user.language/user.country
    // JVM properties).
    javac_core::util::javac_messages::set_default_locale(locale.clone());

    if let Some(option) = info_option(&expanded) {
        match option {
            JavacOption::VERSION | JavacOption::FULLVERSION => {
                println!("javac 17.0.19");
            }
            JavacOption::HELP_LINT => {
                let messages =
                    JavacMessages::with_locale(JAVAC_BUNDLE_NAME, Some(locale.clone()));
                print!("{}", help_lint_text(&messages));
            }
            _ => {
                let messages =
                    JavacMessages::with_locale(JAVAC_BUNDLE_NAME, Some(locale.clone()));
                print!("{}", help_text(&messages, option == JavacOption::X));
            }
        }
        return Ok(OK);
    }

    let mut util_context = javac_core::util::context::Context::new();
    let command_line_log = Log::instance(&mut util_context);
    let mut helper = CommandLineHelper::new(Some(command_line_log));
    let mut index = 0;
    while index < expanded.len() {
        let arg = &expanded[index];
        // --add-opens has no compile-time effect; javac accepts it and emits
        // the [options] warning (the trimmed Option enum omits it).
        if arg == "--add-opens" {
            if index + 1 < expanded.len() {
                helper.put("--add-opens", &expanded[index + 1]);
                index += 2;
                continue;
            }
        } else if let Some(value) = arg.strip_prefix("--add-opens=") {
            helper.put("--add-opens", value);
            index += 1;
            continue;
        }
        // An explicitly empty boot class path makes java.lang unavailable.
        if arg == "-Xbootclasspath:" {
            helper.put("-Xbootclasspath", "");
            index += 1;
            continue;
        }
        // --debug=<name>[=<value>] installs a hidden debug option. The port
        // consumes the same keys as the corresponding -XD options.
        if let Some(value) = arg.strip_prefix("--debug=") {
            let (key, value) = value.split_once('=').unwrap_or((value, "true"));
            let key = format!("debug.{key}");
            helper.put(&key, value);
            index += 1;
            continue;
        }
        // -extdirs is a file-manager option in javac 17 (kept out of the
        // trimmed Option enum); accept and record it for the path warning.
        if arg == "-extdirs" {
            if index + 1 < expanded.len() {
                helper.put("-extdirs", &expanded[index + 1]);
                index += 2;
                continue;
            }
        } else if let Some(value) = arg.strip_prefix("-extdirs:") {
            helper.put("-extdirs", value);
            index += 1;
            continue;
        }
        if let Some(value) = arg
            .strip_prefix("--diags=")
            .or_else(|| arg.strip_prefix("-diags="))
        {
            // Java: --diags=key=value,... is the hidden diagnostic-formatting
            // option; store each key as diags.<key> (the formatter reads
            // diags.formatterOptions and friends).
            for pair in value.split(',') {
                if let Some((key, value)) = pair.split_once('=') {
                    helper.put(&format!("diags.{key}"), value);
                }
            }
            index += 1;
            continue;
        }
        if arg.starts_with("-J") {
            // VM options (-J-Duser.language=...) are not javac options; the
            // locale selection reads user.language/user.country above.
            index += 1;
            continue;
        }
        if arg.starts_with('-') {
            let option = match JavacOption::lookup(arg) {
                Some(option) => option,
                None => {
                    // javac: error: invalid flag: <arg> + usage, exit CMDERR.
                    let messages =
                        JavacMessages::with_locale(JAVAC_BUNDLE_NAME, Some(locale.clone()));
                    eprintln!("error: invalid flag: {arg}");
                    eprintln!(
                        "{}",
                        messages.get_localized_string("javac.msg.usage", &["javac".to_string()])
                    );
                    return Ok(CMDERR);
                }
            };
            // Java: Option.handleOption(helper, arg, rest) (Option.java:1142-1169)
            // consumes the attached/next operand and runs Option.process, so
            // the G_NONE/G_CUSTOM/ANYOF choice values are stored exactly as
            // javac stores them (e.g. "-g:source" and "-g").
            let mut rest = expanded[index + 1..].iter().cloned();
            let remaining_before = rest.clone().count();
            if let Err(error) = option.handle_option(&mut helper, arg, &mut rest) {
                let messages =
                    JavacMessages::with_locale(JAVAC_BUNDLE_NAME, Some(locale.clone()));
                eprintln!("error: {}", error.get_message());
                eprintln!(
                    "{}",
                    messages.get_localized_string("javac.msg.usage", &["javac".to_string()])
                );
                return Ok(CMDERR);
            }
            let consumed = remaining_before - rest.count();
            index += 1 + consumed;
        } else {
            helper.add_file(&javac_fs::Path::from_path_buf(PathBuf::from(arg)));
            index += 1;
        }
    }

    // Java: Option.XSTDOUT.process (Option.java:553-564): redirect every
    // diagnostic to the named file (Log.setWriters) before compilation starts.
    if let Some(path) = helper.get(JavacOption::XSTDOUT) {
        match std::fs::File::create(&path) {
            Ok(file) => {
                let writer: javac_core::util::log::WriterHandle = Rc::new(RefCell::new(
                    Box::new(javac_core::util::log::LineSeparatorWriter::new(file))
                        as Box<dyn std::io::Write>,
                ));
                helper
                    .get_log()
                    .expect("command line log")
                    .set_writers(writer);
            }
            Err(error) => {
                let info = javac_core::resources::compiler_properties::errors::error_writing_file(
                    javac_strings::JavaString::from(path.as_str()),
                    javac_strings::JavaString::from(error.to_string().as_str()),
                );
                let exception = helper.new_invalid_value_exception(info);
                let messages =
                    JavacMessages::with_locale(JAVAC_BUNDLE_NAME, Some(locale.clone()));
                eprintln!("error: {}", exception.get_message());
                eprintln!(
                    "{}",
                    messages.get_localized_string("javac.msg.usage", &["javac".to_string()])
                );
                return Ok(CMDERR);
            }
        }
    }

    let mut sources: Vec<PathBuf> = Vec::new();
    let mut class_names: Vec<String> = Vec::new();
    for path in &helper.files {
        if !path.exists() && path.extension().is_none() {
            class_names.push(path.display().to_string());
            continue;
        }
        if !path.exists() {
            // Java: Main/javac reports an explicitly named missing source as
            // "error: file not found: <path>" followed by the usage line, with
            // the CMDERR exit code (JavacFileManager/Arguments).
            let messages = JavacMessages::with_locale(DEFAULT_BUNDLE_NAME, Some(locale.clone()));
            eprintln!(
                "error: {}",
                messages.get_localized_string(
                    "compiler.err.file.not.found",
                    &[path.display().to_string()],
                )
            );
            let javac_messages =
                JavacMessages::with_locale(JAVAC_BUNDLE_NAME, Some(locale.clone()));
            eprintln!(
                "{}",
                javac_messages.get_localized_string("javac.msg.usage", &["javac".to_string()])
            );
            return Ok(CMDERR);
        }
        collect_sources(path, &mut sources)?;
    }
    // Java: with -m, javac compiles the module's sources discovered on
    // --module-source-path (ModuleFinder); no explicit file arguments are
    // expected in that mode.
    if let Some((_, module_value)) = helper.options.iter().find(|(name, _)| name == "--module")
        && let Some((_, module_source_path)) = helper
            .options
            .iter()
            .find(|(name, _)| name == "--module-source-path")
    {
        for module in module_value.split(',') {
            if module.is_empty() {
                continue;
            }
            for entry in module_source_path.split(';').filter(|entry| !entry.is_empty()) {
                let module_dir = PathBuf::from(entry).join(module);
                if module_dir.is_dir() {
                    collect_sources(&module_dir, &mut sources)?;
                }
            }
        }
    }
    if !class_names.is_empty() {
        let messages = JavacMessages::with_locale(DEFAULT_BUNDLE_NAME, Some(locale.clone()));
        eprintln!(
            "error: {}",
            messages.get_localized_string(
                "compiler.err.proc.no.explicit.annotation.processing.requested",
                &[class_names.join(",")],
            )
        );
        eprintln!("1 error");
        return Ok(ERROR);
    }
    // javac: without -d the class file is written next to its source file
    // (JavacFileManager.java:911-923); CLASS_OUTPUT stays unset.
    let out_dir = helper
        .options
        .iter()
        .find(|(name, _)| name == "-d")
        .map(|(_, value)| PathBuf::from(value));
    for (name, value) in &helper.options {
        if name == "--target" && javac_core::jvm::target::Target::lookup(value).is_none() {
            let messages = JavacMessages::with_locale(JAVAC_BUNDLE_NAME, Some(locale.clone()));
            eprintln!("error: invalid target release: {value}");
            eprintln!(
                "{}",
                messages.get_localized_string("javac.msg.usage", &["javac".to_string()])
            );
            return Ok(CMDERR);
        }
        if name == "--source" && Source::lookup(value).is_none() {
            let messages = JavacMessages::with_locale(JAVAC_BUNDLE_NAME, Some(locale.clone()));
            eprintln!("error: invalid source release: {value}");
            eprintln!(
                "{}",
                messages.get_localized_string("javac.msg.usage", &["javac".to_string()])
            );
            return Ok(CMDERR);
        }
        if name == "--add-modules" {
            for module in value.split(',') {
                if module.is_empty()
                    || matches!(
                        module,
                        "ALL-MODULE-PATH" | "ALL-DEFAULT" | "ALL-SYSTEM"
                    )
                {
                    continue;
                }
                if !is_valid_module_name(module) {
                    // Java: Arguments.checkName (bad name in value for option).
                    let messages =
                        JavacMessages::with_locale(DEFAULT_BUNDLE_NAME, Some(locale.clone()));
                    let text = messages.get_localized_string(
                        "compiler.err.bad.name.for.option",
                        &["--add-modules".to_string(), module.to_string()],
                    );
                    eprintln!("error: {text}");
                    return Ok(CMDERR);
                }
            }
        }
    }
    let options = helper.options.clone();
    // Java: Arguments.validate - -m requires -d and --module-source-path.
    if options.iter().any(|(key, _)| key == "--module") {
        if out_dir.is_none() {
            let messages = JavacMessages::with_locale(DEFAULT_BUNDLE_NAME, Some(locale.clone()));
            let text = messages.get_localized_string(
                "compiler.err.output.dir.must.be.specified.with.dash.m.option",
                &[],
            );
            eprintln!("error: {text}");
            return Ok(CMDERR);
        }
        if !options
            .iter()
            .any(|(key, _)| key == "--module-source-path")
        {
            let messages = JavacMessages::with_locale(DEFAULT_BUNDLE_NAME, Some(locale.clone()));
            let text = messages.get_localized_string(
                "compiler.err.modulesourcepath.must.be.specified.with.dash.m.option",
                &[],
            );
            eprintln!("error: {text}");
            return Ok(CMDERR);
        }
    }
    // Java: Arguments.validate - each -m module must be found in the module
    // source path (the simple, non-wildcard form is checked here; wildcard
    // layouts are resolved by the file manager).
    if let Some((_, module_value)) = options.iter().find(|(key, _)| key == "--module") {
        if let Some((_, module_source_path)) = options
            .iter()
            .find(|(key, _)| key == "--module-source-path")
        {
            let entries: Vec<&str> = module_source_path
                .split(';')
                .filter(|entry| !entry.is_empty())
                .collect();
            let all_simple = entries.iter().all(|entry| !entry.contains('*'));
            if all_simple {
                for module in module_value.split(',') {
                    if module.is_empty() {
                        continue;
                    }
                    let found = entries.iter().any(|entry| {
                        let root = PathBuf::from(entry);
                        root.join(module).join("module-info.java").exists()
                            || root.join(module).is_dir()
                    });
                    if !found {
                        let messages =
                            JavacMessages::with_locale(DEFAULT_BUNDLE_NAME, Some(locale.clone()));
                        let text = messages.get_localized_string(
                            "compiler.err.module.not.found.in.module.source.path",
                            &[module.to_string()],
                        );
                        eprintln!("error: {text}");
                        return Ok(CMDERR);
                    }
                }
            }
        }
    }
    // Java: Arguments.validate - avoid confusion between directories of
    // modules and exploded module directories.
    if let Some(out_dir) = &out_dir
        && options
            .iter()
            .any(|(key, _)| key == "--module-source-path")
        && out_dir.join("module-info.class").exists()
    {
        let messages = JavacMessages::with_locale(DEFAULT_BUNDLE_NAME, Some(locale.clone()));
        let text = messages.get_localized_string(
            "compiler.err.multi-module.outdir.cannot.be.exploded.module",
            &[out_dir.display().to_string()],
        );
        eprintln!("error: {text}");
        return Ok(CMDERR);
    }
    if let Some((_, path)) = options
        .iter()
        .find(|(key, _)| key == "--module-source-path")
    {
        for entry in path.split(';').filter(|entry| !entry.is_empty()) {
            // Java: '*' is only legal as a whole path element or a final /*
            // segment (module-source-path syntax).
            let star_ok = entry == "*" || entry.ends_with("/*") || entry.ends_with("\\*");
            if entry.contains('*') && !star_ok {
                eprintln!(
                    "error: illegal argument for --module-source-path: illegal use of * in {entry}"
                );
                return Ok(CMDERR);
            }
        }
    }
    if options
        .iter()
        .any(|(key, value)| key == "-Xbootclasspath" && value.is_empty())
    {
        eprintln!("Fatal Error: Unable to find package java.lang in classpath or bootclasspath");
        return Ok(ERROR);
    }
    let option_warnings = emit_option_warnings(&expanded, &options, &locale);
    let removed_errors = module_option_errors(&options, &locale);
    if !removed_errors.is_empty() {
        for error in removed_errors {
            eprintln!("error: {error}");
        }
        eprintln!("1 error");
        return Ok(ERROR);
    }
    let removed_errors = removed_option_errors(&options, &locale);
    if !removed_errors.is_empty() {
        for error in removed_errors {
            eprintln!("error: {error}");
        }
        return Ok(CMDERR);
    }

    let has_non_vm_argument = expanded.iter().any(|arg| !arg.starts_with("-J"));
    if sources.is_empty() && !has_non_vm_argument {
        // javac 17 without any arguments prints the full standard help on
        // stdout and exits with CMDERR (verified against
        // JAVA_HOME/bin/javac: 68 stdout lines, exit 2). With options but no
        // source files javac reports no.source.files instead.
        let javac_messages =
            JavacMessages::with_locale(JAVAC_BUNDLE_NAME, Some(locale.clone()));
        print!("{}", help_text(&javac_messages, false));
        return Ok(CMDERR);
    }

    if let Some(out_dir) = &out_dir {
        std::fs::create_dir_all(out_dir)
            .map_err(|error| format!("cannot create directory {}: {error}", out_dir.display()))?;
    }
    let class_output = out_dir.as_deref().unwrap_or(Path::new(""));
    let mut compiler = MiniCompiler::new_with_options(class_output, |options_service| {
        for (name, value) in &options {
            if name == "-d" {
                continue;
            }
            options_service.put(name.clone(), value.clone());
        }
    });

    if sources.is_empty() {
        let messages = JavacMessages::with_locale(DEFAULT_BUNDLE_NAME, Some(locale.clone()));
        eprintln!(
            "error: {}",
            messages.get_localized_string("compiler.err.no.source.files", &[])
        );
        return Ok(ERROR);
    }

    // Java: javac's Log reports diagnostics immediately through its default
    // handler; the count feeds JavaCompiler's shouldStop checks.
    let result = compiler.compile_files(&sources);
    // Java: JavaCompiler.compile calls preview.reportDeferredDiagnostics()
    // after the last round (the preview filename / recompile notes).
    compiler.log.report_deferred_diagnostics();
    compiler.preview.report_deferred_diagnostics();
    compiler.report_deferred_mandatory_diagnostics();
    // Java: JavaCompiler.compile emits the compressed-diagnostics note after
    // the deferred diagnostics report (JavaCompiler.java:1719-1721).
    if compiler.log.compressed_output.get() {
        let note = javac_core::util::jc_diagnostic::Note::from(
            javac_core::resources::compiler_properties::notes::compressed_diags(),
        );
        compiler.log.emit_mandatory_note(note);
    }
    let messages = Rc::new(JavacMessages::with_locale(
        DEFAULT_BUNDLE_NAME,
        Some(locale.clone()),
    ));
    let werror = options
        .iter()
        .any(|(key, _)| key == "-Werror" || key == "Werror");
    let warnings_before_summary = compiler.warning_count() + option_warnings;
    let mut errors = compiler.error_count();
    if werror && errors == 0 && warnings_before_summary > 0 {
        eprintln!("error: warnings found and -Werror specified");
        errors = 1;
    }
    if errors > 0 {
        let key = if errors == 1 {
            "compiler.misc.count.error"
        } else {
            "compiler.misc.count.error.plural"
        };
        compiler
            .log
            .print_raw_lines(&messages.get_localized_string(key, &[errors.to_string()]));
        let suppressed = compiler.suppressed_error_count();
        if suppressed > 0 {
            compiler.log.print_raw_lines(&messages.get_localized_string(
                "compiler.misc.count.error.recompile",
                &[errors.to_string(), (errors + suppressed).to_string()],
            ));
        }
    }
    let warnings = compiler.warning_count() + option_warnings;
    if warnings > 0 {
        let key = if warnings == 1 {
            "compiler.misc.count.warn"
        } else {
            "compiler.misc.count.warn.plural"
        };
        compiler
            .log
            .print_raw_lines(&messages.get_localized_string(key, &[warnings.to_string()]));
        let suppressed = compiler.suppressed_warning_count();
        if suppressed > 0 {
            compiler.log.print_raw_lines(&messages.get_localized_string(
                "compiler.misc.count.warn.recompile",
                &[warnings.to_string(), (warnings + suppressed).to_string()],
            ));
        }
    }
    if errors > 0 || result.is_err() {
        return Ok(ERROR);
    }
    Ok(OK)
}
