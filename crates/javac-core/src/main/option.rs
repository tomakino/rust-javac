// Gap registration (method-level audit of the ported upstream class):
// Dependency/phase: dependent service (see docs/PLAN.md).
// Not yet ported upstream methods (no substitutes in this file):
// - NOT PORTED: getSupportedRuntimeOptions (Option.java:820)
// - NOT PORTED: showHelp (Option.java:1003)
// - NOT PORTED: getPrimaryName (Option.java:1071)
// End of Gap registration.

//! Options for javac.
//! The specific Option to handle a command-line option can be found by calling
//! {@link #lookup}, which search some or all of the members of this enum in order,
//! looking for the first {@link #matches match}.
//! The action for an Option is performed {@link #handleOption}, which determines
//! whether an argument is needed and where to find it;
//! {@code handleOption} then calls {@link #process process} providing a suitable
//! {@link OptionHelper} to provide access the compiler state.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own
//! risk.  This code and its internal interfaces are subject to change
//! or deletion without notice.</b></p>
//!
//!
//! * Java enum -> Rust enum; every constant's constructor data (names, argsNameKey,
//!   descrKey, OptionKind, OptionGroup, ArgKind, ChoiceKind, choices) becomes
//!   match-based accessors (the code/TypeTag pattern).  The primary name is
//!   names()[0] like upstream; Java's public fields are accessors here.  Java enum
//!   name() is reproduced by Display (the constant name), while getKind/getArgKind
//!   keep their upstream accessor names.
//! * Java LinkedHashSet<String> choices -> Vec<String> (insertion order kept); the
//!   dynamic XLINT_CUSTOM choices are built from the ported Lint.LintCategory
//!   (all + the 31 categories with their negated forms + none).
//! * Java Set<Option> -> &[Option] / Vec<Option> (EnumSet order = declaration order).
//! * Java Option.getPattern()/findSeparator()/help()/helpSynopsis()/process()/
//!   handleOption() are NOT declared in this:
//!   REGISTERED(main/OptionHelper + Arguments + Log): process/handleOption/help
//!   need main.OptionHelper, util.Log, java.util.regex and
//!   java.lang.module.ModuleDescriptor, none of which is ported.  No substitute
//!   helper is invented (registered Porting rules).  Option value validation (Source/Target/
//!   Profile lookup, --release platform lookup, choices bookkeeping, the
//!   FILEMANAGER handleFileManagerOption call) is ported together with that,
//!   as is CommandLine/Arguments (the argv scanner) and SOURCEFILE's Files/Paths
//!   checks.  getPattern is deferred to the java.util.regex adapter.
//! * options whose value is a choice keep the upstream choice semantics in
//!   matches(): ONEOF compares the whole attached argument, ANYOF splits on ",+"
//!   (Java String.split semantics for runs of commas: empty input yields [""],
//!   a string of only commas yields []).
//!
//! ## Trimmed options (docs/SUPPORT_MATRIX.md "command-line options / trimming"; no substitute
//! behaviour is invented and these constants do not exist in this enum):
//! * annotation processing (APT is out of scope): PROC (-proc:), PROCESSOR,
//!   PROCESSOR_PATH, PROCESSOR_MODULE_PATH, A (-A).
//! * javadoc-only / deprecated javadoc options: BOOT_CLASS_PATH,
//!   XBOOTCLASSPATH_PREPEND (-Xbootclasspath/p:), XBOOTCLASSPATH_APPEND
//!   (-Xbootclasspath/a:), XBOOTCLASSPATH (-Xbootclasspath:), EXTDIRS,
//!   DJAVA_EXT_DIRS (-Djava.ext.dirs=), ENDORSEDDIRS, DJAVA_ENDORSED_DIRS
//!   (-Djava.endorsed.dirs=).
//! * -h / -s / -J*: H (native header output), S (source output, APT), J (JVM args;
//!   the launcher handles them in upstream javac — no launcher is ported).
//! * doclint (-Xdoclint*, com.sun.tools.doclint is trimmed): XDOCLINT,
//!   XDOCLINT_CUSTOM, XDOCLINT_PACKAGE.
//! * other internal -X/debug/hidden options: HELP_LINT (--help-lint), MOREINFO,
//!   PROMPT, DOE, PRINTSOURCE, WARNUNCHECKED, XSTDOUT, XPRINT, XPRINTROUNDS,
//!   XPRINTPROCESSORINFO, XPREFER, XXUSERPATHSFIRST, XPKGINFO (and the nested
//!   PkgInfo enum), O, XJCOV, PLUGIN (-Xplugin:), DEBUG (--debug), SHOULDSTOP
//!   (--should-stop), DIAGS (--diags; the -XD back door XD is kept because the
//!   differential harness needs -XDshould-stop.at), ADD_OPENS (--add-opens),
//!   LIMIT_MODULES (--limit-modules), MODULE_VERSION (--module-version),
//!   DEFAULT_MODULE_FOR_CREATED_FILES, INHERIT_RUNTIME_ENVIRONMENT.
//! * --multi-release (MULTIRELEASE) is kept (restored after the task-48 trim: the upstream
//!   BaseFileManager.handleOption MULTIRELEASE branch needs it 1:1; multi-release jars are
//!   P8 scope).
//! * NOTE: -XdrawDiagnostics (SUPPORT_MATRIX keep list) never existed as an Option
//!   constant upstream either; it is handled elsewhere (the -XD back door / Log),
//!   so nothing is trimmed for it here.
//! * The upstream counts are: 83 enum constants upstream (verified against the
//!   reference file), 39 kept here, 44 trimmed (listed above).

use std::fmt;
use std::option::Option as StdOption;

use javac_strings::JavaString;
use javac_tools::lang::model::SourceVersion;

use crate::code::lint::LintCategory;

/// Options for javac.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Option {
    G,
    G_NONE,
    G_CUSTOM,
    XLINT,
    XLINT_CUSTOM,
    NOWARN,
    VERBOSE,
    DEPRECATION,
    CLASS_PATH,
    SOURCE_PATH,
    MODULE_SOURCE_PATH,
    MODULE_PATH,
    UPGRADE_MODULE_PATH,
    SYSTEM,
    PATCH_MODULE,
    PARAMETERS,
    D,
    IMPLICIT,
    ENCODING,
    SOURCE,
    TARGET,
    RELEASE,
    PREVIEW,
    VERSION,
    FULLVERSION,
    HELP,
    X,
    WERROR,
    XMAXERRS,
    XMAXWARNS,
    XDIAGS,
    XD,
    ADD_EXPORTS,
    ADD_READS,
    MODULE,
    ADD_MODULES,
    LIMIT_MODULES,
    MODULE_VERSION,
    AT,
    SOURCEFILE,
    H,
    MULTIRELEASE,
    DOE,
    PROMPT,
    HELP_LINT,
    XSTDOUT,
}

impl Option {
    /// Java: Option.values() (the trimmed constants are absent, see the header).
    pub fn values() -> [Option; 44] {
        [
            Option::G,
            Option::G_NONE,
            Option::G_CUSTOM,
            Option::XLINT,
            Option::XLINT_CUSTOM,
            Option::NOWARN,
            Option::VERBOSE,
            Option::DEPRECATION,
            Option::CLASS_PATH,
            Option::SOURCE_PATH,
            Option::MODULE_SOURCE_PATH,
            Option::MODULE_PATH,
            Option::UPGRADE_MODULE_PATH,
            Option::SYSTEM,
            Option::PATCH_MODULE,
            Option::PARAMETERS,
            Option::D,
            Option::IMPLICIT,
            Option::ENCODING,
            Option::SOURCE,
            Option::TARGET,
            Option::RELEASE,
            Option::PREVIEW,
            Option::VERSION,
            Option::FULLVERSION,
            Option::HELP,
            Option::X,
            Option::WERROR,
            Option::XMAXERRS,
            Option::XMAXWARNS,
            Option::XDIAGS,
            Option::XD,
            Option::ADD_EXPORTS,
            Option::ADD_READS,
            Option::MODULE,
            Option::ADD_MODULES,
            Option::LIMIT_MODULES,
            Option::MODULE_VERSION,
            Option::AT,
            Option::SOURCEFILE,
            Option::H,
            Option::MULTIRELEASE,
            Option::HELP_LINT,
            Option::XSTDOUT,
        ]
    }

    /// Java: public final String[] names (the primary name and the aliases).
    pub fn names(self) -> &'static [&'static str] {
        match self {
            Option::G => &["-g"],
            Option::G_NONE => &["-g:none"],
            Option::G_CUSTOM => &["-g:"],
            Option::H => &["-h"],
            Option::XLINT => &["-Xlint"],
            Option::XLINT_CUSTOM => &["-Xlint:"],
            Option::NOWARN => &["-nowarn"],
            Option::VERBOSE => &["-verbose"],
            Option::DEPRECATION => &["-deprecation"],
            Option::CLASS_PATH => &["--class-path", "-classpath", "-cp"],
            Option::SOURCE_PATH => &["--source-path", "-sourcepath"],
            Option::MODULE_SOURCE_PATH => &["--module-source-path"],
            Option::MODULE_PATH => &["--module-path", "-p"],
            Option::UPGRADE_MODULE_PATH => &["--upgrade-module-path"],
            Option::SYSTEM => &["--system"],
            Option::PATCH_MODULE => &["--patch-module"],
            Option::PARAMETERS => &["-parameters"],
            Option::D => &["-d"],
            Option::IMPLICIT => &["-implicit:"],
            Option::ENCODING => &["-encoding"],
            Option::SOURCE => &["--source", "-source"],
            Option::TARGET => &["--target", "-target"],
            Option::RELEASE => &["--release"],
            Option::PREVIEW => &["--enable-preview"],
            Option::VERSION => &["--version", "-version"],
            Option::FULLVERSION => &["--full-version", "-fullversion"],
            Option::HELP => &["--help", "-help", "-?"],
            Option::X => &["--help-extra", "-X"],
            Option::WERROR => &["-Werror"],
            Option::XMAXERRS => &["-Xmaxerrs"],
            Option::XMAXWARNS => &["-Xmaxwarns"],
            Option::XDIAGS => &["-Xdiags:"],
            Option::XD => &["-XD"],
            Option::ADD_EXPORTS => &["--add-exports"],
            Option::ADD_READS => &["--add-reads"],
            Option::MODULE => &["--module", "-m"],
            Option::ADD_MODULES => &["--add-modules"],
            Option::LIMIT_MODULES => &["--limit-modules"],
            Option::MODULE_VERSION => &["--module-version"],
            Option::AT => &["@"],
            Option::SOURCEFILE => &["sourcefile"],
            Option::MULTIRELEASE => &["--multi-release"],
            Option::DOE => &["-doe"],
            Option::PROMPT => &["-prompt"],
            Option::HELP_LINT => &["--help-lint"],
            Option::XSTDOUT => &["-Xstdout"],
        }
    }

    /// Java: public final String primaryName (names[0]).
    pub fn primary_name(self) -> &'static str {
        self.names()[0]
    }

    /// Java: protected final String argsNameKey (null -> None).
    pub fn args_name_key(self) -> StdOption<&'static str> {
        match self {
            Option::XLINT_CUSTOM => Some("opt.arg.Xlint"),
            Option::CLASS_PATH => Some("opt.arg.path"),
            Option::SOURCE_PATH => Some("opt.arg.path"),
            Option::H => Some("opt.arg.directory"),
            Option::MODULE_SOURCE_PATH => Some("opt.arg.mspath"),
            Option::MODULE_PATH => Some("opt.arg.path"),
            Option::UPGRADE_MODULE_PATH => Some("opt.arg.path"),
            Option::SYSTEM => Some("opt.arg.jdk"),
            Option::PATCH_MODULE => Some("opt.arg.patch"),
            Option::D => Some("opt.arg.directory"),
            Option::ENCODING => Some("opt.arg.encoding"),
            Option::SOURCE => Some("opt.arg.release"),
            Option::TARGET => Some("opt.arg.release"),
            Option::RELEASE => Some("opt.arg.release"),
            Option::XMAXERRS => Some("opt.arg.number"),
            Option::XMAXWARNS => Some("opt.arg.number"),
            Option::ADD_EXPORTS => Some("opt.arg.addExports"),
            Option::ADD_READS => Some("opt.arg.addReads"),
            Option::MODULE => Some("opt.arg.m"),
            Option::ADD_MODULES => Some("opt.arg.addmods"),
            Option::LIMIT_MODULES => Some("opt.arg.limitmods"),
            Option::MODULE_VERSION => Some("opt.arg.module.version"),
            Option::AT => Some("opt.arg.file"),
            Option::MULTIRELEASE => Some("opt.arg.multi-release"),
            Option::XSTDOUT => Some("opt.arg.file"),
            _ => None,
        }
    }

    /// Java: protected final String descrKey (null -> None; the upstream null
    /// descrKey entries are the hidden options FULLVERSION/XD/SOURCEFILE).
    pub fn descr_key(self) -> StdOption<&'static str> {
        match self {
            Option::G => Some("opt.g"),
            Option::G_NONE => Some("opt.g.none"),
            Option::G_CUSTOM => Some("opt.g.lines.vars.source"),
            Option::H => Some("opt.headerDest"),
            Option::XLINT => Some("opt.Xlint"),
            Option::XLINT_CUSTOM => Some("opt.Xlint.custom"),
            Option::NOWARN => Some("opt.nowarn"),
            Option::VERBOSE => Some("opt.verbose"),
            Option::DEPRECATION => Some("opt.deprecation"),
            Option::CLASS_PATH => Some("opt.classpath"),
            Option::SOURCE_PATH => Some("opt.sourcepath"),
            Option::MODULE_SOURCE_PATH => Some("opt.modulesourcepath"),
            Option::MODULE_PATH => Some("opt.modulepath"),
            Option::UPGRADE_MODULE_PATH => Some("opt.upgrademodulepath"),
            Option::SYSTEM => Some("opt.system"),
            Option::PATCH_MODULE => Some("opt.patch"),
            Option::PARAMETERS => Some("opt.parameters"),
            Option::D => Some("opt.d"),
            Option::IMPLICIT => Some("opt.implicit"),
            Option::ENCODING => Some("opt.encoding"),
            Option::SOURCE => Some("opt.source"),
            Option::TARGET => Some("opt.target"),
            Option::RELEASE => Some("opt.release"),
            Option::PREVIEW => Some("opt.preview"),
            Option::VERSION => Some("opt.version"),
            Option::FULLVERSION => None,
            Option::HELP => Some("opt.help"),
            Option::X => Some("opt.X"),
            Option::WERROR => Some("opt.Werror"),
            Option::XMAXERRS => Some("opt.maxerrs"),
            Option::XMAXWARNS => Some("opt.maxwarns"),
            Option::XDIAGS => Some("opt.diags"),
            Option::XD => None,
            Option::ADD_EXPORTS => Some("opt.addExports"),
            Option::ADD_READS => Some("opt.addReads"),
            Option::MODULE => Some("opt.m"),
            Option::ADD_MODULES => Some("opt.addmods"),
            Option::LIMIT_MODULES => Some("opt.limitmods"),
            Option::MODULE_VERSION => Some("opt.module.version"),
            Option::AT => Some("opt.AT"),
            Option::SOURCEFILE => None,
            Option::MULTIRELEASE => Some("opt.multi-release"),
            Option::DOE | Option::PROMPT => None,
            Option::HELP_LINT => Some("opt.help.lint"),
            Option::XSTDOUT => Some("opt.Xstdout"),
        }
    }

    /// Java: public OptionKind getKind().
    pub fn get_kind(self) -> OptionKind {
        match self {
            Option::G
            | Option::G_NONE
            | Option::G_CUSTOM
            | Option::NOWARN
            | Option::VERBOSE
            | Option::DEPRECATION
            | Option::CLASS_PATH
            | Option::SOURCE_PATH
            | Option::MODULE_SOURCE_PATH
            | Option::MODULE_PATH
            | Option::UPGRADE_MODULE_PATH
            | Option::SYSTEM
            | Option::PARAMETERS
            | Option::D
            | Option::IMPLICIT
            | Option::ENCODING
            | Option::SOURCE
            | Option::TARGET
            | Option::RELEASE
            | Option::PREVIEW
            | Option::HELP
            | Option::X
            | Option::AT
            | Option::MODULE
            | Option::ADD_MODULES
            | Option::LIMIT_MODULES
            | Option::MODULE_VERSION
            | Option::H => OptionKind::STANDARD,
            Option::XLINT
            | Option::XLINT_CUSTOM
            | Option::PATCH_MODULE
            | Option::XMAXERRS
            | Option::XMAXWARNS
            | Option::XDIAGS
            | Option::ADD_EXPORTS
            | Option::ADD_READS
            | Option::HELP_LINT
            | Option::XSTDOUT => OptionKind::EXTENDED,
            Option::FULLVERSION
            | Option::XD
            | Option::SOURCEFILE
            | Option::MULTIRELEASE
            | Option::DOE
            | Option::PROMPT => OptionKind::HIDDEN,
            Option::VERSION | Option::WERROR => OptionKind::STANDARD,
        }
    }

    /// Java: final OptionGroup group (package-private field; exposed to the crate).
    pub(crate) fn group(self) -> OptionGroup {
        match self {
            Option::G
            | Option::G_NONE
            | Option::G_CUSTOM
            | Option::XLINT
            | Option::XLINT_CUSTOM
            | Option::NOWARN
            | Option::VERBOSE
            | Option::DEPRECATION
            | Option::PARAMETERS
            | Option::IMPLICIT
            | Option::SOURCE
            | Option::TARGET
            | Option::RELEASE
            | Option::PREVIEW
            | Option::WERROR
            | Option::XMAXERRS
            | Option::XMAXWARNS
            | Option::XDIAGS
            | Option::XD
            | Option::ADD_EXPORTS
            | Option::ADD_READS
            | Option::MODULE
            | Option::ADD_MODULES
            | Option::LIMIT_MODULES
            | Option::MODULE_VERSION
            | Option::DOE
            | Option::PROMPT => OptionGroup::BASIC,
            Option::CLASS_PATH
            | Option::SOURCE_PATH
            | Option::MODULE_SOURCE_PATH
            | Option::MODULE_PATH
            | Option::UPGRADE_MODULE_PATH
            | Option::SYSTEM
            | Option::PATCH_MODULE
            | Option::D
            | Option::ENCODING
            | Option::MULTIRELEASE
            | Option::H => OptionGroup::FILEMANAGER,
            Option::VERSION
            | Option::FULLVERSION
            | Option::HELP
            | Option::X
            | Option::AT
            | Option::SOURCEFILE
            | Option::HELP_LINT
            | Option::XSTDOUT => OptionGroup::INFO,
        }
    }

    /// Java: public ArgKind getArgKind().
    pub fn get_arg_kind(self) -> ArgKind {
        match self {
            Option::G
            | Option::G_NONE
            | Option::XLINT
            | Option::NOWARN
            | Option::VERBOSE
            | Option::DEPRECATION
            | Option::PARAMETERS
            | Option::PREVIEW
            | Option::VERSION
            | Option::FULLVERSION
            | Option::HELP
            | Option::X
            | Option::WERROR
            | Option::XD
            | Option::SOURCEFILE
            | Option::HELP_LINT => ArgKind::NONE,
            Option::AT => ArgKind::ADJACENT,
            _ => ArgKind::REQUIRED,
        }
    }

    /// Java: public boolean hasArg().
    pub fn has_arg(self) -> bool {
        self.get_arg_kind() != ArgKind::NONE
    }

    /// Java: public boolean hasSeparateArg().
    pub fn has_separate_arg(self) -> bool {
        self.get_arg_kind() == ArgKind::REQUIRED
            && !self.primary_name().ends_with(':')
            && !self.primary_name().ends_with('=')
    }

    /// Java: private final ChoiceKind choiceKind.
    fn choice_kind(self) -> StdOption<ChoiceKind> {
        match self {
            Option::G_CUSTOM | Option::XLINT_CUSTOM => Some(ChoiceKind::ANYOF),
            Option::IMPLICIT | Option::XDIAGS => Some(ChoiceKind::ONEOF),
            _ => None,
        }
    }

    /// Java: private final Set<String> choices.
    /// Java: protected final Set<String> choices (exposed for help rendering).
    pub fn choices(self) -> StdOption<Vec<String>> {
        match self {
            Option::G_CUSTOM => Some(vec![
                "lines".to_string(),
                "vars".to_string(),
                "source".to_string(),
            ]),
            Option::XLINT_CUSTOM => Some(get_xlint_choices()),
            Option::IMPLICIT => Some(vec!["none".to_string(), "class".to_string()]),
            Option::XDIAGS => Some(vec!["compact".to_string(), "verbose".to_string()]),
            _ => None,
        }
    }

    /// Java: public boolean matches(String option).
    pub fn matches(self, option: &str) -> bool {
        match self {
            // Java: return arg.startsWith("-A") — trimmed with the A option.
            // Java: return s.startsWith(primaryName) — XD.
            Option::XD => return option.starts_with(self.primary_name()),
            // Java: SOURCEFILE.matches — a Java source file name or a legal type name.
            Option::SOURCEFILE => return source_file_matches(option),
            _ => {}
        }
        for name in self.names() {
            if self.matches_name(option, name) {
                return true;
            }
        }
        false
    }

    /// Java: private boolean matches(String option, String name).
    fn matches_name(self, option: &str, name: &str) -> bool {
        if name.starts_with("--") {
            return option == name
                || (self.has_arg() && option.starts_with(&format!("{name}=")));
        }

        let has_suffix =
            self.get_arg_kind() == ArgKind::ADJACENT || name.ends_with(':') || name.ends_with('=');
        if !has_suffix {
            return option == name;
        }
        let Some(attached) = option.strip_prefix(name) else {
            return false;
        };
        if let Some(choices) = self.choices() {
            match self.choice_kind() {
                Some(ChoiceKind::ONEOF) => return choices.iter().any(|choice| choice == attached),
                _ => {
                    // Java: for (String a : arg.split(",+")) {
                    // if (!choices.contains(a)) return false; }
                    for part in split_commas_plus(attached) {
                        if !choices.iter().any(|choice| choice == part) {
                            return false;
                        }
                    }
                }
            }
        }
        true
    }

    /// Java: public static Option lookup(String arg).
    pub fn lookup(arg: &str) -> StdOption<Option> {
        Option::lookup_in(arg, &Option::values())
    }

    /// Java: public static Option lookup(String arg, Set<Option> options).
    ///
    /// Java overload renamed (Porting rules); EnumSet order = declaration order.
    pub fn lookup_in(arg: &str, options: &[Option]) -> StdOption<Option> {
        options.iter().copied().find(|option| option.matches(arg))
    }

    /// Java: static Set<Option> getJavaCompilerOptions().
    pub fn get_java_compiler_options() -> Vec<Option> {
        Option::values().to_vec()
    }

    /// Java: public static Set<Option> getJavacFileManagerOptions().
    pub fn get_javac_file_manager_options() -> Vec<Option> {
        Option::get_options(OptionGroup::FILEMANAGER)
    }

    /// Java: public static Set<Option> getJavacToolOptions().
    pub fn get_javac_tool_options() -> Vec<Option> {
        Option::get_options(OptionGroup::BASIC)
    }

    /// Java: private static Set<Option> getOptions(OptionGroup group).
    fn get_options(group: OptionGroup) -> Vec<Option> {
        Option::values()
            .iter()
            .copied()
            .filter(|option| option.group() == group)
            .collect()
    }
}

impl fmt::Display for Option {
    /// Java: Enum.toString() -> the constant name.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// Java: SOURCEFILE.matches(String s).
fn source_file_matches(option: &str) -> bool {
    if option.ends_with(".java") {
        return true;
    }
    match option.find('/') {
        Some(separator) => {
            let before = JavaString::from(&option[..separator]);
            let after = JavaString::from(&option[separator + 1..]);
            SourceVersion::is_name(&before) && SourceVersion::is_name(&after)
        }
        None => SourceVersion::is_name(&JavaString::from(option)),
    }
}

/// Java: String.split(",+") for the choice-argument split.
///
/// Java semantics: runs of commas separate the values; trailing empty strings are
/// dropped; an empty input yields one empty string; an input consisting only of
/// commas yields an empty array.
fn split_commas_plus(arg: &str) -> Vec<&str> {
    let parts: Vec<&str> = arg.split(',').filter(|part| !part.is_empty()).collect();
    if parts.is_empty() && arg.is_empty() {
        vec![""]
    } else {
        parts
    }
}

/// Java: private static Set<String> getXLintChoices().
///
/// LinkedHashSet order: "all", then each LintCategory option and
/// its negated form in enum declaration order, then "none".
fn get_xlint_choices() -> Vec<String> {
    let mut choices = Vec::new();
    choices.push("all".to_string());
    for category in LintCategory::values() {
        choices.push(category.option().to_string());
        choices.push(format!("-{}", category.option()));
    }
    choices.push("none".to_string());
    choices
}

/// This exception is thrown when an invalid value is given for an option.
/// The detail string gives a detailed, localized message, suitable for use
/// in error messages reported to the user.
///
/// Java: public static class InvalidValueException extends Exception
///
/// REGISTERED(main/OptionHelper): the upstream (String, Throwable) constructor is
/// not ported; the project error model carries the cause separately (Porting rules).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidValueException {
    message: String,
}

impl InvalidValueException {
    /// Java: public InvalidValueException(String msg).
    pub fn new(message: impl Into<String>) -> InvalidValueException {
        InvalidValueException {
            message: message.into(),
        }
    }

    /// Java: Exception.getMessage().
    pub fn get_message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for InvalidValueException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for InvalidValueException {}

/// The kind of argument, if any, accepted by this option.
///
/// Java: public enum ArgKind
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ArgKind {
    /// This option does not take any argument.
    NONE,
    /// This option takes an argument.  If the name of option ends with ':' or '=',
    /// the argument must be provided directly after that separator; otherwise it
    /// may appear after an '=' or in the following argument position.
    REQUIRED,
    /// This option takes an argument immediately after the option name, with no
    /// separator character.
    ADJACENT,
}

impl fmt::Display for ArgKind {
    /// Java: Enum.toString() -> the constant name.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// The kind of an Option. This is used by the -help and -X options.
///
/// Java: public enum OptionKind
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OptionKind {
    /// A standard option, documented by -help.
    STANDARD,
    /// An extended option, documented by -X.
    EXTENDED,
    /// A hidden option, not documented.
    HIDDEN,
}

impl fmt::Display for OptionKind {
    /// Java: Enum.toString() -> the constant name.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// The group for an Option. This determines the situations in which the
/// option is applicable.
///
/// Java: enum OptionGroup (package-private)
///
/// Java constant names kept (BASIC/FILEMANAGER/INFO/OPERAND); no
/// camel-case renaming (Porting rules), so the acronym lint is allowed here.
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum OptionGroup {
    /// A basic option, available for use on the command line or via the
    /// Compiler API.
    BASIC,
    /// An option for javac's standard JavaFileManager.  Other file managers
    /// may or may not support these options.
    FILEMANAGER,
    /// A command-line option that requests information, such as -help.
    INFO,
    /// A command-line "option" representing a file or class name.
    ///
    /// kept 1:1 from upstream; no kept Option constant uses this
    /// group yet (upstream does not either), the Arguments/CommandLine will.
    #[allow(dead_code)]
    OPERAND,
}

/// The kind of choice for "choice" options.
///
/// Java: enum ChoiceKind (package-private)
///
/// Java constant names kept (ONEOF/ANYOF); no camel-case renaming
/// (Porting rules), so the acronym lint is allowed here.
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum ChoiceKind {
    /// The expected value is exactly one of the set of choices.
    ONEOF,
    /// The expected value is one of more of the set of choices.
    ANYOF,
}


// ---------------------------------------------------------------------------
// Java: Option.handleOption/process and Option.InvalidValueException
// (Option.java:1130-1245) — the javacFileManagerOptions path used by
// BaseFileManager.handleOption.
// ---------------------------------------------------------------------------

use crate::main::option_helper::OptionHelper;



impl Option {
    /// Java: private static int findSeparator(String word) (Option.java:1235-1245).
    fn find_separator(word: &str) -> StdOption<usize> {
        word.char_indices()
            .find(|(_, ch)| *ch == ':' || *ch == '=')
            .map(|(index, _)| index)
    }

    /// Java: public void handleOption(OptionHelper helper, String arg,
    /// Iterator<String> rest) (Option.java:1142-1169).
    pub fn handle_option(
        &self,
        helper: &mut dyn OptionHelper,
        arg: &str,
        rest: &mut dyn Iterator<Item = String>,
    ) -> Result<(), InvalidValueException> {
        if self.has_arg() {
            let (option, operand) = if self.get_arg_kind() == ArgKind::ADJACENT {
                // aliases not supported
                let primary = self.primary_name();
                (primary.to_string(), arg[primary.len()..].to_string())
            } else if let StdOption::Some(sep) = Option::find_separator(arg) {
                (arg[..sep].to_string(), arg[sep + 1..].to_string())
            } else {
                match rest.next() {
                    StdOption::Some(operand) => (arg.to_string(), operand),
                    StdOption::None => {
                        return Err(helper.new_invalid_value_exception(
                            crate::resources::compiler_properties::errors::req_arg(JavaString::from(
                                self.primary_name(),
                            )),
                        ));
                    }
                }
            };
            self.process(helper, &option, &operand)
        } else {
            // Java: avoid processing the info options repeatedly.
            if matches!(
                self,
                Option::HELP | Option::X | Option::VERSION | Option::FULLVERSION
            ) && helper.get(*self).is_some()
            {
                return Ok(());
            }
            self.process_no_arg(helper, arg)
        }
    }

    /// Java: public void process(OptionHelper helper, String option)
    /// (Option.java:1178-1185).
    fn process_no_arg(
        &self,
        helper: &mut dyn OptionHelper,
        option: &str,
    ) -> Result<(), InvalidValueException> {
        if self.get_arg_kind() == ArgKind::NONE {
            self.process(helper, self.primary_name(), option)
        } else {
            let sep = Option::find_separator(option).expect("separator in option with an argument");
            self.process(helper, self.primary_name(), &option[sep + 1..])
        }
    }

    /// Java: public void process(OptionHelper helper, String option, String arg)
    /// (Option.java:1195-1218) plus the upstream FILEMANAGER overrides for the
    /// option constants kept by the port (MODULE_SOURCE_PATH/PATCH_MODULE;
    /// BOOT_CLASS_PATH/XBOOTCLASSPATH/EXTDIRS/ENDORSEDDIRS are not ported, see
    /// the file header).
    fn process(
        &self,
        helper: &mut dyn OptionHelper,
        option: &str,
        arg: &str,
    ) -> Result<(), InvalidValueException> {
        use crate::resources::compiler_properties::errors;
        match self {
            Option::G_NONE => {
                // Java: Option.G_NONE.process (Option.java:99-104): -g:none is
                // stored under the "-g:" custom key so ClassWriter sees the
                // G_CUSTOM choice (emitSourceFile/line/vars decisions).
                helper.put("-g:", "none");
                Ok(())
            }
            Option::MODULE_SOURCE_PATH => {
                // Option.java:167-208.
                if arg.is_empty() {
                    return Err(helper.new_invalid_value_exception(errors::no_value_for_option(
                        JavaString::from(option),
                    )));
                }
                match helper.get(Option::MODULE_SOURCE_PATH) {
                    StdOption::None => self.default_process(helper, arg),
                    StdOption::Some(prev) => {
                        if let StdOption::Some(arg_module) = module_specific_form(arg) {
                            let repeated = prev
                                .split('\0')
                                .filter_map(module_specific_form)
                                .any(|module| module == arg_module);
                            if repeated {
                                return Err(helper.new_invalid_value_exception(
                                    errors::repeated_value_for_module_source_path(
                                        JavaString::from(arg_module),
                                    ),
                                ));
                            }
                            self.default_process(helper, &format!("{prev}\0{arg}"));
                        } else {
                            let present = prev
                                .split('\0')
                                .any(|entry| module_specific_form(entry).is_none());
                            if present {
                                return Err(helper.new_invalid_value_exception(
                                    errors::multiple_values_for_module_source_path(),
                                ));
                            }
                            self.default_process(helper, &format!("{prev}\0{arg}"));
                        }
                    }
                }
                Ok(())
            }
            Option::XD => {
                // Option.java XD.process: split "key=value" and store the
                // undocumented option under its key (e.g. -XDforcePreview).
                let arg = arg.strip_prefix("-XD").unwrap_or(arg);
                let (key, value) = match arg.find('=') {
                    Some(eq) => (&arg[..eq], &arg[eq + 1..]),
                    None => (arg, arg),
                };
                helper.put(key, value);
                Ok(())
            }
            Option::PATCH_MODULE => {
                // Option.java:216-251.
                if arg.is_empty() {
                    return Err(helper.new_invalid_value_exception(errors::no_value_for_option(
                        JavaString::from(option),
                    )));
                }
                match patch_module_form(arg) {
                    StdOption::Some(arg_module_package) => {
                        match helper.get(Option::PATCH_MODULE) {
                            StdOption::None => self.default_process(helper, arg),
                            StdOption::Some(prev) => {
                                let repeated = prev
                                    .split('\0')
                                    .filter_map(patch_module_form)
                                    .any(|module| module == arg_module_package);
                                if repeated {
                                    return Err(helper.new_invalid_value_exception(
                                        errors::repeated_value_for_patch_module(JavaString::from(
                                            arg_module_package,
                                        )),
                                    ));
                                }
                                self.default_process(helper, &format!("{prev}\0{arg}"));
                            }
                        }
                        Ok(())
                    }
                    StdOption::None => Err(helper.new_invalid_value_exception(
                        errors::bad_value_for_option(JavaString::from(option), JavaString::from(arg)),
                    )),
                }
            }
            _ => {
                self.default_process(helper, arg);
                Ok(())
            }
        }
    }

    /// Java: the default Option.process body (Option.java:1195-1218).
    fn default_process(&self, helper: &mut dyn OptionHelper, arg: &str) {
        if let StdOption::Some(choices) = self.choices() {
            match self.choice_kind() {
                StdOption::Some(ChoiceKind::ONEOF) => {
                    let primary = self.primary_name();
                    for choice in &choices {
                        helper.remove(&format!("{primary}{choice}"));
                    }
                    let opt = format!("{primary}{arg}");
                    helper.put(&opt, &opt);
                    let name = &primary[..primary.len() - 1];
                    helper.put(name, arg);
                }
                _ => {
                    for a in arg.split(',').filter(|part| !part.is_empty()) {
                        let opt = format!("{}{}", self.primary_name(), a);
                        helper.put(&opt, &opt);
                    }
                }
            }
        }
        helper.put(self.primary_name(), arg);
        if self.group() == OptionGroup::FILEMANAGER {
            helper.handle_file_manager_option(*self, arg);
        }
    }
}

/// Java: MODULE_SOURCE_PATH.getPattern() = Pattern.compile("([\\p{Alnum}$_.]+)=(.*)")
/// The java.util.regex Pattern is not ported; this function implements the
/// exact character-class match (registered adapter).
fn module_specific_form(arg: &str) -> StdOption<&str> {
    let sep = arg.find('=')?;
    let module = &arg[..sep];
    if module.is_empty() {
        return StdOption::None;
    }
    if module
        .chars()
        .all(|ch| ch.is_alphanumeric() || matches!(ch, '$' | '_' | '.'))
    {
        StdOption::Some(module)
    } else {
        StdOption::None
    }
}

/// Java: PATCH_MODULE.getPattern() = Pattern.compile("([^/]+)=(,*[^,].*)")
/// (registered java.util.regex adapter).
fn patch_module_form(arg: &str) -> StdOption<&str> {
    let sep = arg.find('=')?;
    let module = &arg[..sep];
    if module.is_empty() || module.contains('/') {
        return StdOption::None;
    }
    let value = &arg[sep + 1..];
    let rest = value.trim_start_matches(',');
    if rest.is_empty() || rest.starts_with(',') {
        return StdOption::None;
    }
    StdOption::Some(module)
}

#[cfg(test)]
mod tests {
    use super::{ArgKind, InvalidValueException, Option, OptionKind};
    use crate::code::lint::LintCategory;

    #[test]
    fn kept_options_are_complete_and_in_declaration_order() {
        let values = Option::values();
        assert_eq!(values.len(), 44);
        assert_eq!(values[0], Option::G);
        assert_eq!(values[values.len() - 1], Option::XSTDOUT);
        // 83 upstream constants - 44 kept = 39 trimmed (see the file header).
        assert_eq!(44 + 39, 83);
    }

    #[test]
    fn primary_names_and_aliases_match_upstream() {
        assert_eq!(Option::G.primary_name(), "-g");
        assert_eq!(Option::CLASS_PATH.names(), &["--class-path", "-classpath", "-cp"]);
        assert_eq!(Option::CLASS_PATH.primary_name(), "--class-path");
        assert_eq!(Option::SOURCE.names(), &["--source", "-source"]);
        assert_eq!(Option::HELP.names(), &["--help", "-help", "-?"]);
        assert_eq!(Option::MODULE.names(), &["--module", "-m"]);
        assert_eq!(Option::AT.primary_name(), "@");
        assert_eq!(
            Option::MULTIRELEASE.primary_name(),
            "--multi-release"
        );
        for option in Option::values() {
            assert!(!option.names().is_empty());
            assert_eq!(option.names()[0], option.primary_name());
        }
    }

    #[test]
    fn documentation_keys_match_upstream() {
        assert_eq!(Option::G.args_name_key(), None);
        assert_eq!(Option::G.descr_key(), Some("opt.g"));
        assert_eq!(Option::G_CUSTOM.args_name_key(), None);
        assert_eq!(Option::G_CUSTOM.descr_key(), Some("opt.g.lines.vars.source"));
        assert_eq!(Option::XLINT_CUSTOM.args_name_key(), Some("opt.arg.Xlint"));
        assert_eq!(Option::XLINT_CUSTOM.descr_key(), Some("opt.Xlint.custom"));
        assert_eq!(Option::CLASS_PATH.args_name_key(), Some("opt.arg.path"));
        assert_eq!(Option::CLASS_PATH.descr_key(), Some("opt.classpath"));
        // upstream descrKey null entries
        assert_eq!(Option::FULLVERSION.descr_key(), None);
        assert_eq!(Option::XD.descr_key(), None);
        assert_eq!(Option::SOURCEFILE.descr_key(), None);
        assert_eq!(Option::AT.args_name_key(), Some("opt.arg.file"));
        assert_eq!(
            Option::MULTIRELEASE.args_name_key(),
            Some("opt.arg.multi-release")
        );
        assert_eq!(Option::MULTIRELEASE.descr_key(), Some("opt.multi-release"));
    }

    #[test]
    fn kinds_match_upstream() {
        assert_eq!(Option::G.get_kind(), OptionKind::STANDARD);
        assert_eq!(Option::XLINT.get_kind(), OptionKind::EXTENDED);
        assert_eq!(Option::X.get_kind(), OptionKind::STANDARD);
        assert_eq!(Option::VERSION.get_kind(), OptionKind::STANDARD);
        assert_eq!(Option::FULLVERSION.get_kind(), OptionKind::HIDDEN);
        assert_eq!(Option::XD.get_kind(), OptionKind::HIDDEN);
        assert_eq!(Option::WERROR.get_kind(), OptionKind::STANDARD);
        assert_eq!(Option::SOURCEFILE.get_kind(), OptionKind::HIDDEN);
        assert_eq!(Option::MULTIRELEASE.get_kind(), OptionKind::HIDDEN);
        assert_eq!(Option::XMAXERRS.get_kind(), OptionKind::EXTENDED);
    }

    #[test]
    fn option_groups_partition_the_kept_options() {
        let tool = Option::get_javac_tool_options();
        let file_manager = Option::get_javac_file_manager_options();
        assert_eq!(tool.len(), 25);
        assert_eq!(file_manager.len(), 11);
        assert!(tool.iter().all(|option| !file_manager.contains(option)));
        // BASIC + FILEMANAGER + INFO = all kept options.
        assert_eq!(tool.len() + file_manager.len(), 36);
        assert_eq!(Option::get_java_compiler_options().len(), 44);
        assert!(Option::get_java_compiler_options().contains(&Option::VERSION));
        assert!(Option::get_java_compiler_options().contains(&Option::SOURCEFILE));
        assert_eq!(Option::get_javac_file_manager_options()[0], Option::CLASS_PATH);
        assert_eq!(Option::get_javac_tool_options()[0], Option::G);
    }

    #[test]
    fn arg_kinds_and_separate_args_match_upstream() {
        assert_eq!(Option::G.get_arg_kind(), ArgKind::NONE);
        assert!(!Option::G.has_arg());
        assert_eq!(Option::CLASS_PATH.get_arg_kind(), ArgKind::REQUIRED);
        assert!(Option::CLASS_PATH.has_arg());
        assert!(Option::CLASS_PATH.has_separate_arg());
        assert!(!Option::G_CUSTOM.has_separate_arg());
        assert!(!Option::XLINT_CUSTOM.has_separate_arg());
        assert!(!Option::XD.has_separate_arg());
        assert_eq!(Option::AT.get_arg_kind(), ArgKind::ADJACENT);
        assert!(Option::AT.has_arg());
        assert!(!Option::AT.has_separate_arg());
        assert_eq!(Option::SOURCE.get_arg_kind(), ArgKind::REQUIRED);
    }

    #[test]
    fn options_match_their_names() {
        assert!(Option::G.matches("-g"));
        assert!(!Option::G.matches("-g:none"));
        assert!(Option::G_NONE.matches("-g:none"));
        assert!(Option::CLASS_PATH.matches("-cp"));
        assert!(Option::CLASS_PATH.matches("--class-path"));
        assert!(Option::CLASS_PATH.matches("--class-path=classes"));
        assert!(!Option::CLASS_PATH.matches("-cpclasses"));
        assert!(Option::SOURCE.matches("--source"));
        assert!(Option::SOURCE.matches("-source"));
        assert!(Option::SOURCE.matches("--source=17"));
        assert!(!Option::SOURCE.matches("-source=17"));
        assert!(Option::HELP.matches("-?"));
        assert!(Option::X.matches("-X"));
        assert!(Option::X.matches("--help-extra"));
    }

    #[test]
    fn choice_options_validate_the_attached_argument() {
        assert!(Option::G_CUSTOM.matches("-g:lines"));
        assert!(Option::G_CUSTOM.matches("-g:lines,vars,source"));
        assert!(!Option::G_CUSTOM.matches("-g:foo"));
        assert!(Option::IMPLICIT.matches("-implicit:none"));
        assert!(Option::IMPLICIT.matches("-implicit:class"));
        assert!(!Option::IMPLICIT.matches("-implicit:foo"));
        assert!(Option::XDIAGS.matches("-Xdiags:compact"));
        assert!(!Option::XDIAGS.matches("-Xdiags:foo"));
        assert!(Option::XLINT_CUSTOM.matches("-Xlint:all"));
        assert!(Option::XLINT_CUSTOM.matches("-Xlint:-rawtypes"));
        assert!(Option::XLINT_CUSTOM.matches("-Xlint:rawtypes,unchecked"));
        assert!(!Option::XLINT_CUSTOM.matches("-Xlint:bogus"));
    }

    #[test]
    fn xlint_choices_cover_all_categories_and_negations() {
        let choices = super::get_xlint_choices();
        assert_eq!(choices.len(), 64);
        assert_eq!(choices[0], "all");
        assert_eq!(choices[1], "auxiliaryclass");
        assert_eq!(choices[2], "-auxiliaryclass");
        assert_eq!(choices[choices.len() - 1], "none");
        assert_eq!(LintCategory::values().len() * 2 + 2, choices.len());
        for category in LintCategory::values() {
            assert!(choices.contains(&category.option().to_string()));
            assert!(choices.contains(&format!("-{}", category.option())));
        }
    }

    #[test]
    fn hidden_backdoors_match_by_prefix() {
        assert!(Option::XD.matches("-XD"));
        assert!(Option::XD.matches("-XDshould-stop.at"));
        assert!(Option::XD.matches("-XDfoo=bar"));
    }

    #[test]
    fn source_file_operand_matches_sources_and_type_names() {
        assert!(Option::SOURCEFILE.matches("Foo.java"));
        assert!(Option::SOURCEFILE.matches("java.lang.String"));
        assert!(Option::SOURCEFILE.matches("Foo/Bar"));
        assert!(!Option::SOURCEFILE.matches("1abc"));
        assert!(!Option::SOURCEFILE.matches("foo bar"));
    }

    #[test]
    fn lookup_returns_the_first_matching_option() {
        assert_eq!(Option::lookup("-g"), Some(Option::G));
        assert_eq!(Option::lookup("-g:none"), Some(Option::G_NONE));
        assert_eq!(Option::lookup("-g:lines"), Some(Option::G_CUSTOM));
        assert_eq!(Option::lookup("-Xlint"), Some(Option::XLINT));
        assert_eq!(Option::lookup("-Xlint:all"), Some(Option::XLINT_CUSTOM));
        assert_eq!(Option::lookup("-Werror"), Some(Option::WERROR));
        assert_eq!(Option::lookup("--help"), Some(Option::HELP));
        assert_eq!(Option::lookup("--version"), Some(Option::VERSION));
        assert_eq!(Option::lookup("-X"), Some(Option::X));
        assert_eq!(Option::lookup("-cp"), Some(Option::CLASS_PATH));
        assert_eq!(Option::lookup("--class-path=x"), Some(Option::CLASS_PATH));
        assert_eq!(Option::lookup("-d"), Some(Option::D));
        assert_eq!(Option::lookup("-encoding"), Some(Option::ENCODING));
        assert_eq!(Option::lookup("-source"), Some(Option::SOURCE));
        assert_eq!(Option::lookup("--target=17"), Some(Option::TARGET));
        assert_eq!(Option::lookup("--release"), Some(Option::RELEASE));
        assert_eq!(Option::lookup("--enable-preview"), Some(Option::PREVIEW));
        assert_eq!(Option::lookup("-Xmaxerrs"), Some(Option::XMAXERRS));
        assert_eq!(Option::lookup("-Xdiags:verbose"), Some(Option::XDIAGS));
        assert_eq!(Option::lookup("-XDfoo"), Some(Option::XD));
        assert_eq!(Option::lookup("Foo.java"), Some(Option::SOURCEFILE));
        assert_eq!(Option::lookup("-Afoo=bar"), None);
        assert_eq!(
            Option::lookup("--multi-release"),
            Some(Option::MULTIRELEASE)
        );
    }

    #[test]
    fn trimmed_options_are_not_recognized() {
        // SUPPORT_MATRIX trimmed groups: APT, -h/-s/-J, doclint, javadoc
        // deprecated, and the internal -X/debug options.
        for trimmed in [
            "-proc:none",
            "-processor",
            "-processorpath",
            "--processor-module-path",
            "-Akey=value",
            "-s",
            "-J-Xmx1g",
            "-Xdoclint",
            "-Xdoclint:all",
            "-Xdoclint/package:java.lang",
            "-bootclasspath",
            "-Xbootclasspath/p:",
            "-extdirs",
            "-endorseddirs",
            "-Djava.ext.dirs=",
            "-profile",
            "-moreinfo",
            "-doe",
            "-prompt",
            "-printsource",
            "-warnunchecked",
            "-Xprint",
            "-XprintRounds",
            "-XprintProcessorInfo",
            "-Xprefer:source",
            "-XXuserPathsFirst",
            "-Xpkginfo:always",
            "-O",
            "-Xjcov",
            "-Xplugin:name",
            "--debug",
            "--should-stop=at=FLOW",
            "--diags",
            "--add-opens",
            "--default-module-for-created-files",
            "--inherit-runtime-environment",
        ] {
            assert_eq!(Option::lookup(trimmed), None, "{trimmed} must be trimmed");
        }
    }

    #[test]
    fn lookup_in_restricts_the_candidate_set() {
        let candidates = [Option::G, Option::CLASS_PATH];
        assert_eq!(Option::lookup_in("-g", &candidates), Some(Option::G));
        assert_eq!(
            Option::lookup_in("-cp", &candidates),
            Some(Option::CLASS_PATH)
        );
        assert_eq!(Option::lookup_in("-Xlint", &candidates), None);
    }

    use std::collections::HashMap;

    struct RecordingHelper {
        values: HashMap<String, String>,
        file_manager_options: Vec<(Option, String)>,
    }

    impl RecordingHelper {
        fn new() -> RecordingHelper {
            RecordingHelper {
                values: HashMap::new(),
                file_manager_options: Vec::new(),
            }
        }
    }

    impl crate::main::option_helper::OptionHelper for RecordingHelper {
        fn get(&self, option: Option) -> std::option::Option<String> {
            self.values.get(option.primary_name()).cloned()
        }

        fn put(&mut self, name: &str, value: &str) {
            self.values.insert(name.to_string(), value.to_string());
        }

        fn remove(&mut self, name: &str) {
            self.values.remove(name);
        }

        fn handle_file_manager_option(&mut self, option: Option, value: &str) -> bool {
            self.file_manager_options.push((option, value.to_string()));
            true
        }

        fn get_log(
            &self,
        ) -> std::option::Option<std::rc::Rc<std::cell::RefCell<crate::util::log::Log>>> {
            None
        }

        fn get_own_name(&self) -> String {
            "javac".to_string()
        }

        fn add_file(&mut self, _path: &javac_fs::Path) {}

        fn add_class_name(&mut self, _name: &str) {}
    }

    #[test]
    fn module_source_path_composes_and_rejects_duplicate_modules() {
        let mut helper = RecordingHelper::new();
        let mut rest = vec!["src".to_string()].into_iter();
        Option::MODULE_SOURCE_PATH
            .handle_option(&mut helper, "--module-source-path", &mut rest)
            .expect("first value");
        assert_eq!(
            helper
                .values
                .get(Option::MODULE_SOURCE_PATH.primary_name())
                .expect("stored"),
            "src"
        );
        assert_eq!(helper.file_manager_options.len(), 1);

        let mut rest = std::iter::empty();
        Option::MODULE_SOURCE_PATH
            .handle_option(&mut helper, "--module-source-path=m1=src1", &mut rest)
            .expect("module specific value");
        assert!(helper
            .values
            .get(Option::MODULE_SOURCE_PATH.primary_name())
            .expect("stored")
            .contains('\0'));

        let mut rest = std::iter::empty();
        assert!(Option::MODULE_SOURCE_PATH
            .handle_option(&mut helper, "--module-source-path=m1=src2", &mut rest)
            .is_err());
    }

    #[test]
    fn patch_module_rejects_duplicates_and_bad_values() {
        let mut helper = RecordingHelper::new();
        let mut rest = std::iter::empty();
        Option::PATCH_MODULE
            .handle_option(&mut helper, "--patch-module=m1=src", &mut rest)
            .expect("first value");
        let mut rest = std::iter::empty();
        assert!(Option::PATCH_MODULE
            .handle_option(&mut helper, "--patch-module=m1=other", &mut rest)
            .is_err());
        let mut rest = std::iter::empty();
        assert!(Option::PATCH_MODULE
            .handle_option(&mut helper, "--patch-module=invalid", &mut rest)
            .is_err());
    }

    #[test]
    fn encoding_option_requires_an_argument() {
        let mut helper = RecordingHelper::new();
        let mut rest = std::iter::empty();
        assert!(Option::ENCODING
            .handle_option(&mut helper, "-encoding", &mut rest)
            .is_err());
        let mut rest = vec!["UTF-8".to_string()].into_iter();
        Option::ENCODING
            .handle_option(&mut helper, "-encoding", &mut rest)
            .expect("value");
        assert_eq!(
            helper.values.get(Option::ENCODING.primary_name()).map(String::as_str),
            Some("UTF-8")
        );
    }

    #[test]
    fn display_reproduces_the_enum_constant_name() {
        assert_eq!(Option::G_NONE.to_string(), "G_NONE");
        assert_eq!(Option::SOURCEFILE.to_string(), "SOURCEFILE");
        assert_eq!(ArgKind::ADJACENT.to_string(), "ADJACENT");
        assert_eq!(OptionKind::HIDDEN.to_string(), "HIDDEN");
    }

    #[test]
    fn invalid_value_exception_carries_the_localized_message() {
        let exception = InvalidValueException::new("invalid value");
        assert_eq!(exception.get_message(), "invalid value");
        assert_eq!(exception.to_string(), "invalid value");
        assert!(std::error::Error::source(&exception).is_none());
    }
}

/// Java: public enum Option.PkgInfo (-Xpkginfo: values)
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PkgInfo {
    /// Always generate package-info.class for every package-info.java file.
    ALWAYS,
    /// Generate package-info.class if package-info.java contains annotations.
    LEGACY,
    /// Generate package-info.class iff there are annotations to be written.
    NONEMPTY,
}

impl PkgInfo {
    /// Java: public static PkgInfo get(Options options)
    pub fn get(options: &crate::util::options::Options) -> PkgInfo {
        // Java: String v = options.get(XPKGINFO); return (v == null ? PkgInfo.LEGACY
        // : PkgInfo.valueOf(StringUtils.toUpperCase(v)));
        let value = options.get("-Xpkginfo");
        match value {
            None => PkgInfo::LEGACY,
            Some(value) => match value.to_uppercase().as_str() {
                "ALWAYS" => PkgInfo::ALWAYS,
                "LEGACY" => PkgInfo::LEGACY,
                "NONEMPTY" => PkgInfo::NONEMPTY,
                other => panic!("IllegalArgumentException: {other}"),
            },
        }
    }

}