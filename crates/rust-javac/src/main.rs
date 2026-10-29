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
use javac_core::main::mini_compiler::MiniCompiler;
use javac_core::main::option::ArgKind;
use javac_core::main::option::InvalidValueException;
use javac_core::main::option::Option as JavacOption;
use javac_core::main::option::OptionKind;
use javac_core::main::option_helper::OptionHelper;
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
    // The compiler uses deep recursion over types/symbols; the process main
    // thread has a small stack on Windows. Run the driver on a dedicated
    // large-stack thread (javac uses the launcher thread's stack as well).
    let handle = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let args: Vec<String> = std::env::args().skip(1).collect();
            compile(&args)
        })
        .expect("cannot spawn compiler thread");
    let code = match handle.join() {
        Ok(Ok(code)) => code,
        Ok(Err(message)) => {
            eprintln!("javac: {message}");
            CMDERR
        }
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
                JavacOption::HELP | JavacOption::X | JavacOption::VERSION | JavacOption::FULLVERSION
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
    log: Option<Rc<RefCell<Log>>>,
}

impl CommandLineHelper {
    fn new(log: Option<Rc<RefCell<Log>>>) -> CommandLineHelper {
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

    fn get_log(&self) -> Option<Rc<RefCell<Log>>> {
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

fn compile(args: &[String]) -> Result<i32, String> {
    let expanded = match expand_argfiles(args) {
        Ok(expanded) => expanded,
        Err((message, code)) => {
            eprintln!("error: {message}");
            return Ok(code);
        }
    };
    let locale = select_locale(&expanded);

    if let Some(option) = info_option(&expanded) {
        match option {
            JavacOption::VERSION | JavacOption::FULLVERSION => {
                println!("javac 17.0.19");
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

    let mut sources: Vec<PathBuf> = Vec::new();
    for path in &helper.files {
        collect_sources(path, &mut sources)?;
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
    }
    let options = helper.options.clone();

    if sources.is_empty() {
        // javac 17 without source files prints the full standard help on
        // stdout and exits with CMDERR (verified against
        // JAVA_HOME/bin/javac: 68 stdout lines, exit 2).
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

    compiler.log.borrow_mut().install_diagnostic_handler(DiagnosticHandler::Deferred {
        prev: None,
        deferred: VecDeque::new(),
        filter: None,
    });
    let result = compiler.compile_files(&sources);
    let diagnostics = compiler.log.borrow_mut().take_deferred_diagnostics();
    compiler.log.borrow_mut().pop_diagnostic_handler();

    let messages = Rc::new(JavacMessages::with_locale(
        DEFAULT_BUNDLE_NAME,
        Some(locale.clone()),
    ));
    let formatter = BasicDiagnosticFormatter::new(Rc::clone(&messages));
    for diagnostic in &diagnostics {
        eprintln!("{}", formatter.format(diagnostic, None));
    }
    let errors = diagnostics
        .iter()
        .filter(|diagnostic| {
            matches!(
                diagnostic.get_type(),
                javac_core::util::jc_diagnostic::DiagnosticType::ERROR
            )
        })
        .count();
    if errors > 0 {
        let key = if errors == 1 {
            "compiler.misc.count.error"
        } else {
            "compiler.misc.count.error.plural"
        };
        eprintln!("{}", messages.get_localized_string(key, &[errors.to_string()]));
    }
    if errors > 0 || result.is_err() {
        return Ok(ERROR);
    }
    Ok(OK)
}
