//! P4 前置诊断契约:类型化访问器使用的诊断键对象与参数类型(占位实现)。
//!
//! 来源锚点:src/jdk.compiler/share/classes/com/sun/tools/javac/util/JCDiagnostic.java @ 982d53c
//! (jdk-17.0.19-ga)的 DiagnosticInfo / Error / Warning / Note / Fragment;设计见
//! docs/ADR/0002-diagnostic-resources.md。
//!
//! PORTING_RULES: 本文件是 P4(JCDiagnostic/Log 移植)之前的适配占位(登记 §9):
//! - DiagnosticInfo 对应 JCDiagnostic.DiagnosticInfo{type,prefix,code,args};
//! - ErrorInfo/WarningInfo/NoteInfo/FragmentInfo 对应 4 个 JCDiagnostic 诊断子类;
//! - DiagnosticArg 对应 Java 的 Object[] 诊断参数(当前以 Display 文本占位);
//! - 其余 *Arg 类型对应尚未移植的 javac 类型(Symbol/Type/File/...)。
//!
//! P4 替换点(ADR 0002 修订 C):本文件与 xtask/src/gen_resources.rs 的 SIMPLE_TYPES
//! 映射表。替换时必须保留参数的原始对象语义与身份(不得以预先格式化的文本替代参数对象),
//! 验收方式为诊断格式化与 javac 逐字对齐(javac-tests 差分基座字节级比较)。

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
    /// Java: DiagnosticInfo.prefix(资源文件前缀,如 "compiler")。
    pub prefix: &'static str,
    /// Java: DiagnosticInfo.type
    pub kind: DiagnosticType,
    /// Java: DiagnosticInfo.code(如 "plugin.not.found")。
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

/// Java 诊断参数(Object[] 元素)。
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
    /// 以 Display 文本构造占位参数(P4 前的日志/测试辅助)。
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

/// 参数装箱 trait:生成的访问器对每个参数调用 into_diagnostic_arg()。
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

/// 生成 4 个诊断键 newtype(Java: JCDiagnostic 的 4 个诊断子类)。
macro_rules! info_type {
    ($name:ident, $kind:expr) => {
        /// 类型化诊断键(P4 后映射到对应 JCDiagnostic 子类)。
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name(pub DiagnosticInfo);

        impl $name {
            /// 由资源文件生成的访问器构造。
            pub fn new(code: &'static str, args: Vec<DiagnosticArg>) -> $name {
                $name(DiagnosticInfo::new($kind, "compiler", code, args))
            }

            /// 取出底层诊断键对象。
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

/// 生成尚未移植的 javac 诊断参数类型的 opaque 占位(P4 时替换为真实类型)。
macro_rules! opaque_arg {
    ($name:ident) => {
        /// 诊断参数占位类型(P4 时替换为对应的 javac 类型)。
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name(pub String);

        impl $name {
            /// 构造占位值(P4 前的日志/测试辅助)。
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
