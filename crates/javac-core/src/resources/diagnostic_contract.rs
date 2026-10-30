//! P4 diagnostic contract: diagnostic key objects and argument types used by the typed accessors (placeholder implementation).
//!
//! Origin: src/jdk.compiler/share/classes/com/sun/tools/javac/util/JCDiagnostic.java @ 982d53c
//! (jdk-17.0.19-ga): DiagnosticInfo / Error / Warning / Note / Fragment; see
//! docs/ADR/0002-diagnostic-resources.md。
//!
//! PORTING_RULES: this file is the adapter placeholder before P4 (JCDiagnostic/Log port; Porting rules registered):
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
/// PORTING_RULES Porting rules / ADR 0002 P4: the upstream argument is an arbitrary
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
    /// A type argument carrying its TypeId and the metadata needed by the
    /// RichDiagnosticFormatter `where` clauses. The rendered text is kept for
    /// the common formatting path.
    Type(TypeArg),
    /// A diagnostic-argument list (Java: Object[] argument; used where the
    /// message placeholder renders a comma-separated list, for example the
    /// method arguments in cant.apply.symbol).
    List(Vec<DiagnosticArg>),
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
            DiagnosticArg::Text(_) | DiagnosticArg::Type(_) | DiagnosticArg::List(_) => None,
        }
    }

    /// The textual rendering of this argument (Text as-is; a nested diagnostic
    /// through its message — P4 prints the message text, Porting rules registered).
    pub fn to_text(&self) -> String {
        match self {
            DiagnosticArg::Text(text) => text.clone(),
            DiagnosticArg::JCDiagnostic(diag) => diag.get_message(None).to_string(),
            DiagnosticArg::Type(arg) => arg.text.clone(),
            DiagnosticArg::List(items) => items
                .iter()
                .map(|item| item.to_text())
                .collect::<Vec<_>>()
                .join(","),
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
            (DiagnosticArg::Type(a), DiagnosticArg::Type(b)) => a == b,
            (DiagnosticArg::List(a), DiagnosticArg::List(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for DiagnosticArg {}

impl From<&str> for DiagnosticArg {
    fn from(value: &str) -> DiagnosticArg {
        DiagnosticArg::Text(value.to_string())
    }
}

impl From<String> for DiagnosticArg {
    fn from(value: String) -> DiagnosticArg {
        DiagnosticArg::Text(value)
    }
}

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
        // Java: AbstractDiagnosticFormatter.formatIterable formats each element;
        // keep the structured items so TypeArg where metadata survives.
        DiagnosticArg::List(
            self.into_iter()
                .map(IntoDiagnosticArg::into_diagnostic_arg)
                .collect(),
        )
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
/// P4 placeholder: SymbolArg is the pre-formatted text argument used by the
/// typed accessors for symbol parameters (ADR 0002). It shares the
/// DiagnosticArg representation so both accessor call styles are accepted;
/// the real Symbol type replaces it at P4.
pub type SymbolArg = DiagnosticArg;

impl DiagnosticArg {
    /// Build a placeholder argument from Display text (the same helper the
    /// other diagnostic argument types expose before P4).
    pub fn placeholder(text: impl Into<String>) -> Self {
        DiagnosticArg::Text(text.into())
    }
}
opaque_arg!(SymbolKindArg);
opaque_arg!(TargetArg);
opaque_arg!(TokenArg);
opaque_arg!(TreeTagArg);
opaque_arg!(UrlArg);

/// One type-variable entry of a RichDiagnosticFormatter `where` clause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhereTypeVar {
    /// The type-variable display name (for example T).
    pub name: String,
    /// The upper bound display (for example Object).
    pub bound: String,
    /// The declaring symbol display (for example "class X" or
    /// "method <T>m(List<T>)").
    pub owner: String,
    /// Java: the tvar symbol has the SYNTHETIC flag (a fresh type variable;
    /// RichDiagnosticFormatter renders WhereFreshTypevar without a location).
    pub fresh: bool,
}

/// One captured-type entry of a RichDiagnosticFormatter `where` clause. The
/// formatter assigns CAP#N in encounter order; `original` is the printer text
/// used to locate the type inside the rendered argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhereCaptured {
    /// Printer text of the captured type (capture#N of ...).
    pub original: String,
    /// Upper bound display.
    pub upper: String,
    /// Lower bound display (only rendered when it is not the null type).
    pub lower: Option<String>,
    /// Wildcard display (from capture of ...).
    pub wildcard: String,
}

/// One intersection-type entry of a RichDiagnosticFormatter `where` clause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhereIntersection {
    /// Printer text of the intersection type.
    pub original: String,
    /// Supertype followed by interfaces (upstream WhereIntersection order).
    pub bounds: Vec<String>,
}

/// A Type diagnostic argument. The TypeId is preserved for formatter-level
/// processing; `text` is the rendered type and `where_vars` carries the
/// declarations used by the RichDiagnosticFormatter `where` clauses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeArg {
    /// Rendered type text (delegated to code::printer at construction).
    pub text: String,
    /// The Java Type object identity.
    pub ty: Option<crate::ids::TypeId>,
    /// Qualified rendering (Java: Type.toString()) used by the raw formatter.
    pub qualified_text: Option<String>,
    /// Type variables occurring in the type, in encounter order.
    pub where_vars: Vec<WhereTypeVar>,
    /// Captured types occurring in the type, in encounter order.
    pub where_captured: Vec<WhereCaptured>,
    /// Intersection types occurring in the type, in encounter order.
    pub where_intersections: Vec<WhereIntersection>,
}

impl TypeArg {
    /// Build a placeholder value (log/test helper before P4).
    pub fn placeholder(text: impl Into<String>) -> TypeArg {
        TypeArg {
            text: text.into(),
            ty: None,
            qualified_text: None,
            where_vars: Vec::new(),
            where_captured: Vec::new(),
            where_intersections: Vec::new(),
        }
    }

    /// Build a typed argument with its where-clause metadata.
    pub fn with_where(
        text: impl Into<String>,
        ty: crate::ids::TypeId,
        where_vars: Vec<WhereTypeVar>,
    ) -> TypeArg {
        TypeArg {
            text: text.into(),
            ty: Some(ty),
            qualified_text: None,
            where_vars,
            where_captured: Vec::new(),
            where_intersections: Vec::new(),
        }
    }
}

impl From<&str> for TypeArg {
    fn from(text: &str) -> TypeArg {
        TypeArg::placeholder(text)
    }
}

impl From<String> for TypeArg {
    fn from(text: String) -> TypeArg {
        TypeArg::placeholder(text)
    }
}

impl IntoDiagnosticArg for TypeArg {
    fn into_diagnostic_arg(self) -> DiagnosticArg {
        DiagnosticArg::Type(self)
    }
}

/// Convert a contract argument to the formatter's Rc<dyn Any> argument,
/// preserving Type/JCDiagnostic/list objects.
pub(crate) fn to_jc_diagnostic_arg(
    arg: &DiagnosticArg,
) -> crate::util::jc_diagnostic::DiagnosticArg {
    use crate::util::jc_diagnostic::DiagnosticArg as JcDiagnosticArg;
    match arg {
        DiagnosticArg::Text(text) => {
            std::rc::Rc::new(javac_strings::JavaString::from(text.as_str())) as JcDiagnosticArg
        }
        DiagnosticArg::JCDiagnostic(diag) => diag.clone(),
        DiagnosticArg::Type(type_arg) => std::rc::Rc::new(type_arg.clone()),
        DiagnosticArg::List(items) => {
            let converted: Vec<JcDiagnosticArg> = items.iter().map(to_jc_diagnostic_arg).collect();
            std::rc::Rc::new(converted)
        }
    }
}
