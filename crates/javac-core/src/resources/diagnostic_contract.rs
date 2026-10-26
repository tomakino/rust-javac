//! P4 diagnostic contract: diagnostic key objects and argument types used by the typed accessors (placeholder implementation).
//!
//! Origin: src/jdk.compiler/share/classes/com/sun/tools/javac/util/JCDiagnostic.java @ 982d53c
//! (jdk-17.0.19-ga): DiagnosticInfo / Error / Warning / Note / Fragment; see
//! docs/ADR/0002-diagnostic-resources.md。
//!
//! PORTING_RULES: this file is the adapter placeholder before P4 (JCDiagnostic/Log port; §9 registered):
//! - DiagnosticInfo corresponds to JCDiagnostic.DiagnosticInfo{type,prefix,code,args};
//! - ErrorInfo/WarningInfo/NoteInfo/FragmentInfo correspond to the 4 JCDiagnostic subclasses;
//! - DiagnosticArg corresponds to the Java Object[] diagnostic arguments (currently placeholder Display text);
//! - the other *Arg types correspond to javac types not ported yet (Symbol/Type/File/...).
//!
//! P4 replacement points (ADR 0002 revision C): this file and SIMPLE_TYPES in xtask/src/gen_resources.rs
//! mapping. The replacement must keep the original object semantics and identity of the arguments (pre-formatted text must not replace argument objects);
//! acceptance is diagnostic formatting aligned with javac (byte-level comparison in the javac-tests differential base).

use std::fmt;

use crate::util::java_string::JavaString;
use crate::util::name::Name;

/// Java: JCDiagnostic.DiagnosticType
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticType {
    Error,
    Warning,
    Note,
    Fragment,
}

impl DiagnosticType {
    /// Java: DiagnosticType.key
    pub fn key(self) -> &'static str {
        match self {
            DiagnosticType::Error => "err",
            DiagnosticType::Warning => "warn",
            DiagnosticType::Note => "note",
            DiagnosticType::Fragment => "misc",
        }
    }
}

/// Java: JCDiagnostic.DiagnosticInfo
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticInfo {
    /// Java: DiagnosticInfo.prefix (the resource bundle prefix, e.g. "compiler").
    pub prefix: &'static str,
    /// Java: DiagnosticInfo.type
    pub kind: DiagnosticType,
    /// Java: DiagnosticInfo.code (e.g. "plugin.not.found").
    pub code: &'static str,
    /// Java: DiagnosticInfo.args
    pub args: Vec<DiagnosticArg>,
}

impl DiagnosticInfo {
    /// Java: DiagnosticInfo(type, prefix, code, args)
    pub fn new(
        kind: DiagnosticType,
        prefix: &'static str,
        code: &'static str,
        args: Vec<DiagnosticArg>,
    ) -> DiagnosticInfo {
        DiagnosticInfo {
            prefix,
            kind,
            code,
            args,
        }
    }

    /// Java: DiagnosticInfo.key()
    pub fn key(&self) -> String {
        format!("{}.{}.{}", self.prefix, self.kind.key(), self.code)
    }

    /// Java: DiagnosticInfo.getArgs()
    pub fn args(&self) -> &[DiagnosticArg] {
        &self.args
    }
}

/// Java diagnostic argument (an element of the Object[]).
///
/// PORTING_RULES §9 / ADR 0002 P4: the upstream argument is an arbitrary
/// Object; in particular a nested JCDiagnostic can be an argument (the
/// Resolve/AttrRecover nested-diagnostic scans rely on it). The port keeps the
/// formatted Text variant for the existing call sites and adds the
/// JCDiagnostic variant to preserve the nested object (option A of the ADR).
#[derive(Debug, Clone)]
pub enum DiagnosticArg {
    /// A pre-formatted textual argument (legacy P4 placeholder).
    Text(String),
    /// A nested diagnostic argument (Java: a JCDiagnostic in the Object[]).
    JCDiagnostic(std::rc::Rc<crate::util::jc_diagnostic::JCDiagnostic>),
}

impl DiagnosticArg {
    /// Build a placeholder argument from Display text (log/test helper before P4).
    pub fn from_display(value: impl fmt::Display) -> DiagnosticArg {
        DiagnosticArg::Text(value.to_string())
    }

    /// Java: the nested diagnostic argument, if any.
    pub fn nested_diagnostic(&self) -> Option<&std::rc::Rc<crate::util::jc_diagnostic::JCDiagnostic>> {
        match self {
            DiagnosticArg::JCDiagnostic(diag) => Some(diag),
            DiagnosticArg::Text(_) => None,
        }
    }

    /// The textual rendering of this argument (Text as-is; a nested diagnostic
    /// through its message — P4 prints the message text, §9 registered).
    pub fn to_text(&self) -> String {
        match self {
            DiagnosticArg::Text(text) => text.clone(),
            DiagnosticArg::JCDiagnostic(diag) => diag.get_message(None).to_string(),
        }
    }
}

impl PartialEq for DiagnosticArg {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (DiagnosticArg::Text(a), DiagnosticArg::Text(b)) => a == b,
            (DiagnosticArg::JCDiagnostic(a), DiagnosticArg::JCDiagnostic(b)) => {
                std::rc::Rc::ptr_eq(a, b) || a.get_message(None) == b.get_message(None)
            }
            _ => false,
        }
    }
}

impl Eq for DiagnosticArg {}

/// Argument boxing trait: the generated accessors call into_diagnostic_arg() for every argument.
pub trait IntoDiagnosticArg {
    fn into_diagnostic_arg(self) -> DiagnosticArg;
}

impl IntoDiagnosticArg for DiagnosticArg {
    fn into_diagnostic_arg(self) -> DiagnosticArg {
        self
    }
}

impl IntoDiagnosticArg for i32 {
    fn into_diagnostic_arg(self) -> DiagnosticArg {
        DiagnosticArg::Text(self.to_string())
    }
}

impl IntoDiagnosticArg for bool {
    fn into_diagnostic_arg(self) -> DiagnosticArg {
        DiagnosticArg::Text(self.to_string())
    }
}

impl IntoDiagnosticArg for () {
    fn into_diagnostic_arg(self) -> DiagnosticArg {
        DiagnosticArg::Text("()".to_string())
    }
}

impl IntoDiagnosticArg for Name {
    fn into_diagnostic_arg(self) -> DiagnosticArg {
        DiagnosticArg::Text(self.to_string())
    }
}

impl IntoDiagnosticArg for JavaString {
    fn into_diagnostic_arg(self) -> DiagnosticArg {
        DiagnosticArg::Text(self.to_string())
    }
}

impl<T: IntoDiagnosticArg> IntoDiagnosticArg for Vec<T> {
    fn into_diagnostic_arg(self) -> DiagnosticArg {
        let parts: Vec<String> = self
            .into_iter()
            .map(|item| item.into_diagnostic_arg().to_text())
            .collect();
        DiagnosticArg::Text(parts.join(", "))
    }
}

/// Generate the 4 diagnostic-key newtypes (Java: the 4 JCDiagnostic subclasses).
macro_rules! info_type {
    ($name:ident, $kind:expr) => {
        /// Typed diagnostic key (after P4 it maps to the corresponding JCDiagnostic subclass).
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name(pub DiagnosticInfo);

        impl $name {
            /// Constructed by the accessors generated from the resource files.
            pub fn new(code: &'static str, args: Vec<DiagnosticArg>) -> $name {
                $name(DiagnosticInfo::new($kind, "compiler", code, args))
            }

            /// Return the underlying diagnostic-key object.
            pub fn info(&self) -> &DiagnosticInfo {
                &self.0
            }
        }
    };
}

info_type!(ErrorInfo, DiagnosticType::Error);
info_type!(WarningInfo, DiagnosticType::Warning);
info_type!(NoteInfo, DiagnosticType::Note);
info_type!(FragmentInfo, DiagnosticType::Fragment);

/// Generate opaque placeholders for javac diagnostic argument types not ported yet (replaced by the real types at P4).
macro_rules! opaque_arg {
    ($name:ident) => {
        /// Placeholder diagnostic argument type (replaced by the corresponding javac type at P4).
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name(pub String);

        impl $name {
            /// Build a placeholder value (log/test helper before P4).
            pub fn placeholder(text: impl Into<String>) -> $name {
                $name(text.into())
            }
        }

        impl From<&str> for $name {
            fn from(text: &str) -> $name {
                $name(text.to_string())
            }
        }

        impl From<String> for $name {
            fn from(text: String) -> $name {
                $name(text)
            }
        }

        impl IntoDiagnosticArg for $name {
            fn into_diagnostic_arg(self) -> DiagnosticArg {
                DiagnosticArg::Text(self.0)
            }
        }
    };
}

opaque_arg!(AnnotationArg);
opaque_arg!(FileArg);
opaque_arg!(FileObjectArg);
opaque_arg!(FlagArg);
opaque_arg!(KindNameArg);
opaque_arg!(ModifierArg);
opaque_arg!(OptionArg);
opaque_arg!(PathArg);
opaque_arg!(ProfileArg);
opaque_arg!(SourceArg);
opaque_arg!(SourceVersionArg);
opaque_arg!(SymbolArg);
opaque_arg!(SymbolKindArg);
opaque_arg!(TargetArg);
opaque_arg!(TokenArg);
opaque_arg!(TreeTagArg);
opaque_arg!(TypeArg);
opaque_arg!(UrlArg);
