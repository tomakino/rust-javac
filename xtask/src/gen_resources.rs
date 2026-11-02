//! gen-resources: 从上游 javac 资源文件生成 Rust 类型化诊断访问器,并用 javac 源码
//! 调用点做一致性校验。
//!
//! 来源锚点(上游生成器与构建规则,均 @ 982d53c / jdk-17.0.19-ga):
//! - make/langtools/tools/propertiesparser/PropertiesParser.java
//! - make/langtools/tools/propertiesparser/parser/{Message,MessageFile,MessageInfo,MessageLine,MessageType}.java
//! - make/langtools/tools/propertiesparser/gen/ClassGenerator.java
//! - make/langtools/tools/propertiesparser/resources/templates.properties
//! - make/common/modules/GensrcCommonLangtools.gmk (SetupParseProperties)
//! - make/modules/jdk.compiler/Gensrc.gmk
//! - src/jdk.compiler/share/classes/com/sun/tools/javac/util/JCDiagnostic.java
//!
//! 本文件是生成器(测试基础设施/适配工具),不是移植文件;设计见
//! docs/ADR/0002-diagnostic-resources.md。

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use crate::properties::parse_properties;

const COMPILER_PROPERTIES: &str =
    "crates/javac-core/resources/l10n/compiler.properties";
const JAVAC_SOURCE_ROOT: &str =
    "internal/reference/jdk17u/src/jdk.compiler/share/classes/com/sun/tools/javac";
const OUTPUT_ACCESSORS: &str = "crates/javac-core/src/resources/compiler_accessors.rs";
const OUTPUT_METADATA: &str = "crates/javac-core/src/resources/compiler_metadata.rs";
const OUTPUT_REEXPORT: &str = "crates/javac-core/src/resources/compiler_properties.rs";
#[cfg(test)]
const RESOURCE_ROOT: &str =
    "internal/reference/jdk17u/src/jdk.compiler/share/classes/com/sun/tools/javac/resources";
const BASELINE: &str = "982d53c (jdk-17.0.19-ga)";

// Localized bundles are now embedded as raw .properties text via include_str!
// (crates/javac-core/resources/l10n), so the generator only emits the typed
// accessors and their metadata.



// 上游资源快照断言(见 ADR 0002 修订 G)。
const EXPECTED_KEYS: usize = 936;
const EXPECTED_ERRORS: usize = 471;
const EXPECTED_WARNINGS: usize = 129;
const EXPECTED_NOTES: usize = 32;
const EXPECTED_FRAGMENTS: usize = 304;
const EXPECTED_NO_ARG_KEYS: usize = 391;
const EXPECTED_WITH_ARGS_KEYS: usize = 545;

/// 诊断种类(对应 JCDiagnostic.DiagnosticType 与 CompilerProperties 的 4 个嵌套类)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Error,
    Warning,
    Note,
    Fragment,
}

impl Kind {
    fn from_key_segment(segment: &str) -> Option<Kind> {
        match segment {
            "err" => Some(Kind::Error),
            "warn" => Some(Kind::Warning),
            "note" => Some(Kind::Note),
            "misc" => Some(Kind::Fragment),
            _ => None,
        }
    }

    /// Java 生成类里的嵌套类名。
    fn java_namespace(self) -> &'static str {
        match self {
            Kind::Error => "Errors",
            Kind::Warning => "Warnings",
            Kind::Note => "Notes",
            Kind::Fragment => "Fragments",
        }
    }

    /// Rust 生成模块名。
    fn rust_module(self) -> &'static str {
        match self {
            Kind::Error => "errors",
            Kind::Warning => "warnings",
            Kind::Note => "notes",
            Kind::Fragment => "fragments",
        }
    }

    /// Rust 返回类型名。
    fn rust_info_type(self) -> &'static str {
        match self {
            Kind::Error => "ErrorInfo",
            Kind::Warning => "WarningInfo",
            Kind::Note => "NoteInfo",
            Kind::Fragment => "FragmentInfo",
        }
    }

    /// COMPILER_KEYS 元数据里的种类名。
    fn metadata_name(self) -> &'static str {
        match self {
            Kind::Error => "Error",
            Kind::Warning => "Warning",
            Kind::Note => "Note",
            Kind::Fragment => "Fragment",
        }
    }
}

/// 复合类型 kind(Java: MessageType.CompoundType.Kind)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompoundKind {
    Collection,
    List,
    Set,
}

impl CompoundKind {
    fn prefix(self) -> &'static str {
        match self {
            CompoundKind::Collection => "collection of",
            CompoundKind::List => "list of",
            CompoundKind::Set => "set of",
        }
    }
}

/// 资源文件类型注释解析出的类型表达式(Java: MessageType 家族)。
#[derive(Debug, Clone, PartialEq, Eq)]
enum TypeExpr {
    Simple(String),
    Compound(CompoundKind, Box<TypeExpr>),
    Union(Vec<TypeExpr>),
    Custom(String),
    Unknown,
}

/// Java 简单类型表(Java: MessageType.SimpleType 的 kindName -> Rust 契约类型)。
///
/// 注意:file / path / file object 保持独立(ADR 0002 修订 B),即使 P4 前都是 opaque。
const SIMPLE_TYPES: &[(&str, &str)] = &[
    ("annotation", "AnnotationArg"),
    ("boolean", "bool"),
    ("collection", "Vec<DiagnosticArg>"),
    ("diagnostic", "DiagnosticArg"),
    ("file", "FileArg"),
    ("file object", "FileObjectArg"),
    ("flag", "FlagArg"),
    ("fragment", "DiagnosticArg"),
    ("kind name", "KindNameArg"),
    ("list", "Vec<DiagnosticArg>"),
    ("modifier", "ModifierArg"),
    ("name", "Name"),
    ("number", "i32"),
    ("object", "DiagnosticArg"),
    ("option name", "OptionArg"),
    ("path", "PathArg"),
    ("profile", "ProfileArg"),
    ("set", "Vec<DiagnosticArg>"),
    ("source", "SourceArg"),
    ("source version", "SourceVersionArg"),
    ("string", "JavaString"),
    ("symbol", "SymbolArg"),
    ("symbol kind", "SymbolKindArg"),
    ("target", "TargetArg"),
    ("token", "TokenArg"),
    ("tree tag", "TreeTagArg"),
    ("type", "TypeArg"),
    ("unused", "()"),
    ("url", "UrlArg"),
    ("<unknown>", "DiagnosticArg"),
];

/// 一条消息资源(Java: parser.Message)。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    key: String,
    kind: Kind,
    /// 去掉文件前缀与 kind 段后的 code(Java: DiagnosticInfo.code)。
    code: String,
    value: String,
    types: Vec<TypeExpr>,
}

impl Entry {
    fn java_factory_name(&self) -> String {
        java_factory_name(&self.key)
    }

    fn rust_factory_name(&self) -> String {
        rust_factory_name(&self.java_factory_name())
    }

    fn is_no_args(&self) -> bool {
        self.types.is_empty()
    }
}

/// Java: ClassGenerator.factoryName —— key 按 [.-] 切分、跳过前两段、每段首字母大写拼接。
fn java_factory_name(key: &str) -> String {
    key.split(['.', '-'])
        .skip(2)
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// Java 工厂名(CamelCase) -> Rust snake_case 名(数字段保持拼接)。
fn rust_factory_name(java_name: &str) -> String {
    let mut out = String::new();
    for (index, ch) in java_name.chars().enumerate() {
        if ch.is_uppercase() {
            if index > 0 {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// Rust 保留关键字(含 2018/2024 edition 与未稳定的保留字)。
const RUST_KEYWORDS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "crate",
    "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl",
    "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "try",
    "type", "typeof", "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

/// 生成函数名:关键字加 r# 前缀(如 r#static),其余原样。
fn escape_rust_name(name: &str) -> String {
    if RUST_KEYWORDS.contains(&name) {
        format!("r#{name}")
    } else {
        name.to_string()
    }
}

/// 判断一行是否是类型注释行(Java: MessageLine.isInfo 的简化:所有段都是 "N: type")。
fn looks_like_info_line(line: &str) -> bool {
    let Some(body) = line.strip_prefix("# ") else {
        return false;
    };
    body.split(", ").enumerate().all(|(index, segment)| {
        segment
            .split_once(": ")
            .is_some_and(|(prefix, _)| prefix.trim() == index.to_string())
    })
}

/// 解析类型注释行,例如 # 0: kind name, 1: symbol, 2: list of type。
fn parse_info_line(line: &str) -> Result<Vec<TypeExpr>, String> {
    let body = line
        .strip_prefix("# ")
        .ok_or_else(|| format!("type comment must start with '# ': {line}"))?;
    let mut types = Vec::new();
    for (index, segment) in body.split(", ").enumerate() {
        let (prefix, type_text) = segment
            .split_once(": ")
            .ok_or_else(|| format!("malformed type comment segment: {segment}"))?;
        if prefix.trim() != index.to_string() {
            return Err(format!(
                "expected placeholder index {index}, found '{prefix}' in: {line}"
            ));
        }
        types.push(parse_type_expr(type_text.trim()));
    }
    Ok(types)
}

/// Java: MessageInfo.parseType —— 先去掉可选注释,再按 or 分割成联合类型。
fn parse_type_expr(text: &str) -> TypeExpr {
    // Java: int commentStart = text.indexOf("("); if (commentStart != -1) text = text.substring(0, commentStart);
    let text = match text.find('(') {
        Some(position) => text[..position].trim(),
        None => text,
    };
    let alternatives: Vec<&str> = text.split(" or ").collect();
    if alternatives.len() > 1 {
        TypeExpr::Union(
            alternatives
                .iter()
                .map(|alternative| parse_alternative(alternative))
                .collect(),
        )
    } else {
        parse_alternative(text)
    }
}

/// Java: MessageInfo.parseAlternative —— 自定义类型/简单类型/复合类型/联合别名。
fn parse_alternative(text: &str) -> TypeExpr {
    let text = text.trim();
    if let Some(rest) = text.strip_prefix('\'')
        && let Some(end) = rest.find('\'')
    {
        return TypeExpr::Custom(rest[..end].to_string());
    }
    if SIMPLE_TYPES.iter().any(|(name, _)| *name == text) {
        return TypeExpr::Simple(text.to_string());
    }
    for (prefix, kind) in [
        ("collection of", CompoundKind::Collection),
        ("list of", CompoundKind::List),
        ("set of", CompoundKind::Set),
    ] {
        if let Some(rest) = text.strip_prefix(prefix) {
            return TypeExpr::Compound(kind, Box::new(parse_alternative(rest.trim())));
        }
    }
    if text == "message segment" {
        return TypeExpr::Union(vec![
            TypeExpr::Simple("diagnostic".to_string()),
            TypeExpr::Simple("fragment".to_string()),
        ]);
    }
    if text == "file name" {
        return TypeExpr::Union(vec![
            TypeExpr::Simple("file".to_string()),
            TypeExpr::Simple("file object".to_string()),
            TypeExpr::Simple("path".to_string()),
        ]);
    }
    TypeExpr::Unknown
}

/// 类型表达式 -> Rust 契约类型(ADR 0002 的类型映射表)。
fn rust_type(expr: &TypeExpr) -> String {
    match expr {
        TypeExpr::Simple(name) => SIMPLE_TYPES
            .iter()
            .find(|(java_name, _)| *java_name == name.as_str())
            .map(|(_, rust_name)| (*rust_name).to_string())
            .unwrap_or_else(|| "DiagnosticArg".to_string()),
        TypeExpr::Compound(_, element) => format!("Vec<{}>", rust_type(element)),
        TypeExpr::Union(_) | TypeExpr::Custom(_) | TypeExpr::Unknown => {
            "DiagnosticArg".to_string()
        }
    }
}

/// 类型表达式 -> 原始 Java 类型文本(用于文档注释)。
fn type_expr_display(expr: &TypeExpr) -> String {
    match expr {
        TypeExpr::Simple(name) => name.clone(),
        TypeExpr::Compound(kind, element) => {
            format!("{} {}", kind.prefix(), type_expr_display(element))
        }
        TypeExpr::Union(choices) => choices
            .iter()
            .map(type_expr_display)
            .collect::<Vec<_>>()
            .join(" or "),
        TypeExpr::Custom(text) => format!("'{text}'"),
        TypeExpr::Unknown => "<unknown>".to_string(),
    }
}

/// 解析 compiler.properties:键值(Java Properties 语义) + 紧邻上一行的类型注释。
fn parse_entries(text: &str) -> Result<Vec<Entry>, String> {
    let values = parse_properties(text);
    let lines: Vec<&str> = text.lines().collect();
    let mut entries = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if !line.starts_with("compiler.") {
            continue;
        }
        let Some(equals) = line.find('=') else {
            continue;
        };
        let key = line[..equals].trim().to_string();
        if key.is_empty() {
            continue;
        }
        let Some(value) = values.get(&key) else {
            continue;
        };
        let types = if index > 0 {
            let previous = lines[index - 1].trim_end();
            if looks_like_info_line(previous) {
                parse_info_line(previous).map_err(|e| format!("{key}: {e}"))?
            } else {
                // Upstream PropertiesParser generates a zero-argument dummy
                // factory when the value uses {n} without a type comment.
                vec![]
            }
        } else {
            vec![]
        };
        let parts: Vec<&str> = key.split('.').collect();
        if parts.len() < 3 {
            return Err(format!("malformed resource key: {key}"));
        }
        let kind = Kind::from_key_segment(parts[1])
            .ok_or_else(|| format!("{key}: unsupported diagnostic kind '{}'", parts[1]))?;
        let code = parts[2..].join(".");
        let entry = Entry {
            key,
            kind,
            code,
            value: value.clone(),
            types,
        };
        verify_entry_arity(&entry)?;
        entries.push(entry);
    }
    entries.sort_by(|left, right| left.key.cmp(&right.key));
    Ok(entries)
}

/// 修订 G:占位符个数不能超过类型注释段数(注释不足即错误)。
///
/// 注:上游 compiler.properties 有 29 个键的值含 {n} 但紧邻上一行不是类型注释
/// (被空行/普通注释隔开)。upstream PropertiesParser 对它们生成零参工厂(dummyInfo),
/// 本生成器保持同样行为,并在入口处打印警告计数(不失败)。
fn verify_entry_arity(entry: &Entry) -> Result<(), String> {
    let placeholders = placeholder_count(&entry.value);
    if !entry.types.is_empty() && placeholders > entry.types.len() {
        return Err(format!(
            "{}: value uses {placeholders} placeholder(s) but only {} type comment(s)",
            entry.key,
            entry.types.len()
        ));
    }
    Ok(())
}

/// 统计 value 中 {n} 占位符的个数(最大下标 + 1)。
fn placeholder_count(value: &str) -> usize {
    let mut maximum: Option<usize> = None;
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '{' {
            continue;
        }
        let mut digits = String::new();
        while let Some(&next) = chars.peek() {
            if next.is_ascii_digit() {
                digits.push(next);
                chars.next();
            } else {
                break;
            }
        }
        if !digits.is_empty() && chars.peek() == Some(&'}') {
            chars.next();
            if let Ok(index) = digits.parse::<usize>() {
                maximum = Some(maximum.map_or(index, |current| current.max(index)));
            }
        }
    }
    maximum.map_or(0, |index| index + 1)
}

/// 生成访问器源文件(确定性:输入顺序固定 + BTreeMap + 固定换行)。
fn generate_accessors(entries: &[Entry]) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "//! GENERATED by cargo xtask gen-resources from com/sun/tools/javac/resources/compiler.properties @ {BASELINE} \u{2014} DO NOT EDIT."
    );
    out.push_str("//!\n");
    out.push_str("//! Typed accessors for the compiler diagnostics, mirroring the class that\n");
    out.push_str("//! upstream javac generates with propertiesparser.PropertiesParser\n");
    out.push_str("//! (CompilerProperties: nested Errors/Warnings/Notes/Fragments factories).\n");
    out.push_str("//! Argument types follow the ADR 0002 mapping; the names and arities match\n");
    out.push_str("//! the javac-generated factories.\n\n");
    out.push_str("#![allow(clippy::too_many_arguments)]\n\n");
    out.push_str("use super::diagnostic_contract::*;\n");
    out.push_str("use crate::util::java_string::JavaString;\n");
    out.push_str("use crate::util::name::Name;\n\n");

    let mut by_kind: BTreeMap<Kind, Vec<&Entry>> = BTreeMap::new();
    for entry in entries {
        by_kind.entry(entry.kind).or_default().push(entry);
    }
    for (kind, entries) in &by_kind {
        let _ = writeln!(out, "pub mod {} {{", kind.rust_module());
        out.push_str("    use super::*;\n\n");
        for entry in entries {
            let java_name = entry.java_factory_name();
            let rust_name = escape_rust_name(&entry.rust_factory_name());
            let arguments: Vec<String> = entry
                .types
                .iter()
                .enumerate()
                .map(|(index, expr)| format!("arg{index}: {}", rust_type(expr)))
                .collect();
            let argument_docs: Vec<String> = entry.types.iter().map(type_expr_display).collect();
            let doc_value = entry.value.replace('\n', "\\n").replace('\t', "\\t");
            let _ = writeln!(out, "    /// {}", doc_value);
            out.push_str("    ///\n");
            let _ = writeln!(
                out,
                "    /// Java: {}.{}({})",
                kind.java_namespace(),
                java_name,
                argument_docs.join(", ")
            );
            let _ = writeln!(
                out,
                "    pub fn {rust_name}({}) -> {} {{",
                arguments.join(", "),
                kind.rust_info_type()
            );
            let packed: Vec<String> = (0..entry.types.len())
                .map(|index| format!("arg{index}.into_diagnostic_arg()"))
                .collect();
            let _ = writeln!(
                out,
                "        {}::new({:?}, vec![{}])",
                kind.rust_info_type(),
                entry.code,
                packed.join(", ")
            );
            out.push_str("    }\n\n");
        }
        out.push_str("}\n\n");
    }


    out
}

/// 生成访问器元数据源文件(key, kind, Rust 名, 参数个数)。
fn generate_metadata(entries: &[Entry]) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "//! GENERATED by cargo xtask gen-resources from com/sun/tools/javac/resources/compiler.properties @ {BASELINE} \u{2014} DO NOT EDIT."
    );
    out.push_str("//!\n");
    out.push_str("//! Accessor metadata: (resource key, diagnostic kind, Rust factory name, arity).\n");
    out.push_str("//! Kept separate from both the accessor code (compiler_accessors.rs) and\n");
    out.push_str("//! the English message table (compiler_en.rs).\n\n");
    out.push_str("pub static COMPILER_KEYS: &[(&str, &str, &str, usize)] = &[\n");
    for entry in entries {
        let _ = writeln!(
            out,
            "    ({:?}, {:?}, {:?}, {}),",
            entry.key,
            entry.kind.metadata_name(),
            escape_rust_name(&entry.rust_factory_name()),
            entry.types.len()
        );
    }
    out.push_str("];\n");
    out
}

/// 生成 compiler_properties.rs 兼容层:数据表在 compiler_en.rs,访问器在
/// compiler_accessors.rs,元数据在 compiler_metadata.rs;这里只做 re-export。
fn generate_reexport() -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "//! GENERATED by cargo xtask gen-resources from com/sun/tools/javac/resources/compiler.properties @ {BASELINE} \u{2014} DO NOT EDIT."
    );
    out.push_str("//!\n");
    out.push_str("//! Compatibility re-exports: the English message table lives in\n");
    out.push_str("//! resources/l10n (see resources/bundles.rs), the accessor code in\n");
    out.push_str("//! compiler_accessors.rs and the metadata in compiler_metadata.rs.\n\n");
    out.push_str("pub use super::compiler_accessors::{errors, fragments, notes, warnings};\n");
    out.push_str("pub use super::compiler_metadata::COMPILER_KEYS;\n");
    out.push_str("pub use super::bundles::COMPILER_EN as COMPILER_MESSAGES;\n");
    out
}

fn project_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn project_path(relative: &str) -> PathBuf {
    project_root().join(relative)
}

/// 校验上游资源快照的统计(修订 G)。
fn verify_snapshot(entries: &[Entry]) -> Result<(), String> {
    if entries.len() != EXPECTED_KEYS {
        return Err(format!(
            "expected {EXPECTED_KEYS} resource keys, found {}",
            entries.len()
        ));
    }
    let count = |kind: Kind| entries.iter().filter(|entry| entry.kind == kind).count();
    let checks = [
        (Kind::Error, EXPECTED_ERRORS),
        (Kind::Warning, EXPECTED_WARNINGS),
        (Kind::Note, EXPECTED_NOTES),
        (Kind::Fragment, EXPECTED_FRAGMENTS),
    ];
    for (kind, expected) in checks {
        if count(kind) != expected {
            return Err(format!(
                "expected {expected} {kind:?} keys, found {}",
                count(kind)
            ));
        }
    }
    let no_args = entries.iter().filter(|entry| entry.is_no_args()).count();
    if no_args != EXPECTED_NO_ARG_KEYS {
        return Err(format!(
            "expected {EXPECTED_NO_ARG_KEYS} no-argument keys, found {no_args}"
        ));
    }
    let with_args = entries.len() - no_args;
    if with_args != EXPECTED_WITH_ARGS_KEYS {
        return Err(format!(
            "expected {EXPECTED_WITH_ARGS_KEYS} keys with arguments, found {with_args}"
        ));
    }
    Ok(())
}

/// 校验生成名字唯一(Java 生成类与 Rust 模块都要求唯一)。
fn verify_names(entries: &[Entry]) -> Result<(), String> {
    let mut java_names: BTreeMap<(Kind, String), &str> = BTreeMap::new();
    // Rust 名只需要在同一个生成模块(kind)内唯一,不同模块之间允许同名。
    let mut rust_names: BTreeMap<(Kind, String), &str> = BTreeMap::new();
    for entry in entries {
        let java_name = entry.java_factory_name();
        if let Some(existing) = java_names.insert((entry.kind, java_name.clone()), &entry.key) {
            return Err(format!(
                "duplicate Java factory name {}.{java_name} for {} and {}",
                entry.kind.java_namespace(),
                existing,
                entry.key
            ));
        }
        let rust_name = entry.rust_factory_name();
        if let Some(existing) =
            rust_names.insert((entry.kind, rust_name.clone()), &entry.key)
        {
            return Err(format!(
                "duplicate Rust factory name {}::{rust_name} for {} and {}",
                entry.kind.rust_module(),
                existing,
                entry.key
            ));
        }
    }
    Ok(())
}

/// 一个 javac 源码诊断调用点。
#[derive(Debug, Clone, PartialEq, Eq)]
struct CallSite {
    file: String,
    line: usize,
    namespace: String,
    name: String,
    /// Some(n) = 方法调用(参数个数);None = 字段访问(无参)。
    args: Option<usize>,
}

/// 递归收集 .java 文件。
fn collect_java_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(root).map_err(|e| format!("read {}: {e}", root.display()))?;
    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("read entry: {e}"))?;
        paths.push(entry.path());
    }
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_java_files(&path, files)?;
        } else if path.extension().is_some_and(|extension| extension == "java") {
            files.push(path);
        }
    }
    Ok(())
}

/// 扫描 javac 源码里的 (Errors|Warnings|Notes|Fragments).<Name> 调用点。
///
/// 字符状态机跳过字符串/字符/注释;参数个数按括号层级计数(修订 D)。
fn scan_call_sites(root: &Path) -> Result<Vec<CallSite>, String> {
    let mut files = Vec::new();
    collect_java_files(root, &mut files)?;
    let mut sites = Vec::new();
    for file in files {
        let source = fs::read_to_string(&file)
            .map_err(|e| format!("read {}: {e}", file.display()))?;
        // launcher.properties 生成的 LauncherProperties 不在本次生成范围(ADR 0002 后续项),
        // 引用它的启动器文件跳过;其 Errors.* 属于 LauncherProperties.Errors。
        if source.contains("LauncherProperties") {
            continue;
        }
        let file_name = file.to_string_lossy().to_string();
        scan_call_sites_in(&source, &file_name, &mut sites)?;
    }
    Ok(sites)
}

fn scan_call_sites_in(source: &str, file: &str, sites: &mut Vec<CallSite>) -> Result<(), String> {
    let chars: Vec<char> = source.chars().collect();
    let line_starts = line_starts(&chars);
    let mut index = 0usize;
    while index < chars.len() {
        let c = chars[index];
        if c == '/' && chars.get(index + 1) == Some(&'/') {
            index = skip_line_comment(&chars, index);
            continue;
        }
        if c == '/' && chars.get(index + 1) == Some(&'*') {
            index = skip_block_comment(&chars, index)?;
            continue;
        }
        if c == '"' {
            index = skip_string(&chars, index)?;
            continue;
        }
        if c == '\'' {
            index = skip_char_literal(&chars, index)?;
            continue;
        }
        if c.is_ascii_alphabetic() || c == '_' {
            let start = index;
            while index < chars.len()
                && (chars[index].is_ascii_alphanumeric() || chars[index] == '_')
            {
                index += 1;
            }
            let word: String = chars[start..index].iter().collect();
            // 跳过 import 语句(import static ...CompilerProperties.Fragments.Diamond);
            // import 行里的点号引用不是调用点。
            if word == "import" && is_at_line_start(&chars, start) {
                index = skip_line_comment(&chars, start);
                continue;
            }
            let is_namespace = ["Errors", "Warnings", "Notes", "Fragments"]
                .iter()
                .any(|namespace| **namespace == word);
            let preceded_by_identifier = start > 0
                && (chars[start - 1].is_ascii_alphanumeric()
                    || chars[start - 1] == '_'
                    || chars[start - 1] == '$');
            if !is_namespace || preceded_by_identifier {
                continue;
            }
            // 允许命名空间与成员之间的换行/空白(跨行调用)。
            let mut dot = index;
            while dot < chars.len() && chars[dot].is_whitespace() {
                dot += 1;
            }
            if dot >= chars.len() || chars[dot] != '.' {
                continue;
            }
            let mut member = dot + 1;
            while member < chars.len() && chars[member].is_whitespace() {
                member += 1;
            }
            index = member;
            while index < chars.len()
                && (chars[index].is_ascii_alphanumeric() || chars[index] == '_')
            {
                index += 1;
            }
            let name: String = chars[member..index].iter().collect();
            if name.is_empty() {
                continue;
            }
            let mut look = index;
            while look < chars.len() && chars[look].is_whitespace() {
                look += 1;
            }
            let args = if look < chars.len() && chars[look] == '(' {
                Some(count_arguments(&chars, look)?)
            } else {
                None
            };
            sites.push(CallSite {
                file: file.to_string(),
                line: line_of(&line_starts, start),
                namespace: word,
                name,
                args,
            });
            continue;
        }
        index += 1;
    }
    Ok(())
}

/// 判断位置是否位于行首(之前只有空白)。
fn is_at_line_start(chars: &[char], position: usize) -> bool {
    chars[..position]
        .iter()
        .rev()
        .take_while(|c| **c != '\n')
        .all(|c| c.is_whitespace())
}

fn line_starts(chars: &[char]) -> Vec<usize> {
    let mut starts = vec![0usize];
    for (index, c) in chars.iter().enumerate() {
        if *c == '\n' {
            starts.push(index + 1);
        }
    }
    starts
}

fn line_of(line_starts: &[usize], index: usize) -> usize {
    line_starts.partition_point(|start| *start <= index).max(1)
}

fn skip_line_comment(chars: &[char], index: usize) -> usize {
    let mut cursor = index;
    while cursor < chars.len() && chars[cursor] != '\n' {
        cursor += 1;
    }
    cursor
}

fn skip_block_comment(chars: &[char], index: usize) -> Result<usize, String> {
    let mut cursor = index + 2;
    while cursor + 1 < chars.len() {
        if chars[cursor] == '*' && chars[cursor + 1] == '/' {
            return Ok(cursor + 2);
        }
        cursor += 1;
    }
    Err("unterminated block comment".to_string())
}

fn skip_string(chars: &[char], index: usize) -> Result<usize, String> {
    let mut cursor = index + 1;
    while cursor < chars.len() {
        let c = chars[cursor];
        if c == '\\' {
            cursor += 2;
            continue;
        }
        if c == '"' {
            return Ok(cursor + 1);
        }
        cursor += 1;
    }
    Err("unterminated string literal".to_string())
}

fn skip_char_literal(chars: &[char], index: usize) -> Result<usize, String> {
    let mut cursor = index + 1;
    while cursor < chars.len() {
        let c = chars[cursor];
        if c == '\\' {
            cursor += 2;
            continue;
        }
        if c == '\'' {
            return Ok(cursor + 1);
        }
        cursor += 1;
    }
    Err("unterminated character literal".to_string())
}

/// 从 '(' 开始按括号层级统计参数个数(顶层逗号数 + 1)。
///
/// 顶层 = 圆括号深度 1 且花括号/方括号深度都为 0(数组初始化、匿名类、下标都不是
/// 参数分隔符);内嵌调用的逗号在更深的圆括号层级,不计入。
fn count_arguments(chars: &[char], open: usize) -> Result<usize, String> {
    let mut parens = 0usize;
    let mut braces = 0usize;
    let mut brackets = 0usize;
    let mut commas = 0usize;
    let mut has_content = false;
    let mut index = open;
    while index < chars.len() {
        let c = chars[index];
        if c == '/' && chars.get(index + 1) == Some(&'/') {
            index = skip_line_comment(chars, index);
            continue;
        }
        if c == '/' && chars.get(index + 1) == Some(&'*') {
            index = skip_block_comment(chars, index)?;
            continue;
        }
        if c == '"' {
            index = skip_string(chars, index)?;
            has_content = true;
            continue;
        }
        if c == '\'' {
            index = skip_char_literal(chars, index)?;
            has_content = true;
            continue;
        }
        match c {
            '(' => {
                parens += 1;
                index += 1;
            }
            ')' => {
                parens -= 1;
                if parens == 0 {
                    return Ok(if has_content { commas + 1 } else { 0 });
                }
                index += 1;
            }
            '{' => {
                braces += 1;
                index += 1;
            }
            '}' => {
                braces = braces.saturating_sub(1);
                index += 1;
            }
            '[' => {
                brackets += 1;
                index += 1;
            }
            ']' => {
                brackets = brackets.saturating_sub(1);
                index += 1;
            }
            ',' if parens == 1 && braces == 0 && brackets == 0 => {
                commas += 1;
                index += 1;
            }
            _ => {
                if parens >= 1 && !c.is_whitespace() {
                    has_content = true;
                }
                index += 1;
            }
        }
    }
    Err("unbalanced parentheses".to_string())
}

/// 用生成的访问器表校验调用点(名字 + 参数个数)。
fn verify_call_sites(entries: &[Entry], sites: &[CallSite]) -> Result<(), String> {
    let mut table: BTreeMap<(String, String), usize> = BTreeMap::new();
    for entry in entries {
        table.insert(
            (
                entry.kind.java_namespace().to_string(),
                entry.java_factory_name(),
            ),
            entry.types.len(),
        );
    }
    let mut errors = Vec::new();
    let mut distinct_names: BTreeMap<(String, String), usize> = BTreeMap::new();
    for site in sites {
        *distinct_names
            .entry((site.namespace.clone(), site.name.clone()))
            .or_insert(0) += 1;
        let Some(expected) = table.get(&(site.namespace.clone(), site.name.clone())) else {
            errors.push(format!(
                "{}:{}: unknown diagnostic factory {}.{}",
                site.file, site.line, site.namespace, site.name
            ));
            continue;
        };
        match site.args {
            None => {
                if *expected != 0 {
                    errors.push(format!(
                        "{}:{}: {}.{} is a field access but the factory takes {} argument(s)",
                        site.file, site.line, site.namespace, site.name, expected
                    ));
                }
            }
            Some(actual) => {
                if actual != *expected {
                    errors.push(format!(
                        "{}:{}: {}.{} called with {} argument(s), factory has {}",
                        site.file, site.line, site.namespace, site.name, actual, expected
                    ));
                }
            }
        }
    }
    if !errors.is_empty() {
        let mut message = format!(
            "{} call site(s) do not match the generated factories:\n",
            errors.len()
        );
        for error in errors.iter().take(20) {
            message.push_str("  ");
            message.push_str(error);
            message.push('\n');
        }
        return Err(message);
    }
    println!(
        "gen-resources: call sites verified: {} references, {} distinct factories",
        sites.len(),
        distinct_names.len()
    );
    Ok(())
}

/// cargo xtask gen-resources 入口。
pub(crate) fn generate_and_verify() -> Result<(), String> {
    let properties_path = project_path(COMPILER_PROPERTIES);
    let text = fs::read_to_string(&properties_path)
        .map_err(|e| format!("read {}: {e}", properties_path.display()))?;
    let entries = parse_entries(&text)?;
    verify_snapshot(&entries)?;
    verify_names(&entries)?;
    let generated = generate_accessors(&entries);
    let output_path = project_path(OUTPUT_ACCESSORS);
    fs::write(&output_path, &generated)
        .map_err(|e| format!("write {}: {e}", output_path.display()))?;
    let metadata = generate_metadata(&entries);
    let metadata_path = project_path(OUTPUT_METADATA);
    fs::write(&metadata_path, &metadata)
        .map_err(|e| format!("write {}: {e}", metadata_path.display()))?;
    let reexport = generate_reexport();
    let reexport_path = project_path(OUTPUT_REEXPORT);
    fs::write(&reexport_path, &reexport)
        .map_err(|e| format!("write {}: {e}", reexport_path.display()))?;
    // Localized bundles are embedded at compile time (include_str! in
    // resources/l10n/*); the generator no longer emits message tables.
    let source_root = project_path(JAVAC_SOURCE_ROOT);
    if source_root.is_dir() {
        let sites = scan_call_sites(&source_root)?;
        verify_call_sites(&entries, &sites)?;
    } else {
        println!(
            "gen-resources: call site verification skipped: {} not present",
            source_root.display()
        );
    }
    let placeholders_without_types = entries
        .iter()
        .filter(|entry| entry.is_no_args() && placeholder_count(&entry.value) > 0)
        .count();
    if placeholders_without_types > 0 {
        println!(
            "gen-resources: warning: {placeholders_without_types} key(s) use {{n}} placeholders but have no adjacent type comment; upstream PropertiesParser also generates zero-argument factories for them"
        );
    }
    println!(
        "gen-resources: {} keys (err {} / warn {} / note {} / misc {}; no-arg {}), {} bytes -> {}",
        entries.len(),
        entries.iter().filter(|e| e.kind == Kind::Error).count(),
        entries.iter().filter(|e| e.kind == Kind::Warning).count(),
        entries.iter().filter(|e| e.kind == Kind::Note).count(),
        entries.iter().filter(|e| e.kind == Kind::Fragment).count(),
        entries.iter().filter(|e| e.is_no_args()).count(),
        generated.len(),
        output_path.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_type_comments() {
        let types = parse_info_line("# 0: kind name, 1: symbol, 2: list of type").expect("info");
        assert_eq!(types.len(), 3);
        assert_eq!(rust_type(&types[0]), "KindNameArg");
        assert_eq!(rust_type(&types[1]), "SymbolArg");
        assert_eq!(rust_type(&types[2]), "Vec<TypeArg>");

        let types = parse_info_line("# 0: type or symbol").expect("info");
        assert_eq!(rust_type(&types[0]), "DiagnosticArg");

        let types = parse_info_line("# 0: message segment").expect("info");
        assert_eq!(rust_type(&types[0]), "DiagnosticArg");

        let types = parse_info_line("# 0: string (statement type)").expect("info");
        assert_eq!(rust_type(&types[0]), "JavaString");

        let types = parse_info_line("# 0: file, 1: path, 2: file object").expect("info");
        assert_eq!(rust_type(&types[0]), "FileArg");
        assert_eq!(rust_type(&types[1]), "PathArg");
        assert_eq!(rust_type(&types[2]), "FileObjectArg");

        let types = parse_info_line("# 0: unused, 1: unused").expect("info");
        assert_eq!(rust_type(&types[0]), "()");

        let types = parse_info_line("# 0: set of modifier").expect("info");
        assert_eq!(rust_type(&types[0]), "Vec<ModifierArg>");

        let types = parse_info_line("# 0: list of type or message segment").expect("info");
        assert_eq!(rust_type(&types[0]), "DiagnosticArg");
    }

    #[test]
    fn parses_entries_with_arity_and_kinds() {
        let text = "# 0: symbol\ncompiler.err.foo.bar=\\\n    value {0}\n\ncompiler.misc.baz=\\\n    no args\n";
        let entries = parse_entries(text).expect("entries");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].kind, Kind::Error);
        assert_eq!(entries[0].code, "foo.bar");
        assert_eq!(entries[0].types.len(), 1);
        assert!(entries[1].is_no_args());
    }

    #[test]
    fn names_follow_upstream_rules() {
        assert_eq!(
            java_factory_name("compiler.err.plugin.not.found"),
            "PluginNotFound"
        );
        assert_eq!(rust_factory_name("PluginNotFound"), "plugin_not_found");
        assert_eq!(
            java_factory_name("compiler.warn.missing-explicit-ctor"),
            "MissingExplicitCtor"
        );
        assert_eq!(
            rust_factory_name("MissingExplicitCtor"),
            "missing_explicit_ctor"
        );
        assert_eq!(
            java_factory_name("compiler.err.annotation.missing.default.value.1"),
            "AnnotationMissingDefaultValue1"
        );
        assert_eq!(
            rust_factory_name("AnnotationMissingDefaultValue1"),
            "annotation_missing_default_value1"
        );
        assert_eq!(escape_rust_name("static"), "r#static");
        assert_eq!(escape_rust_name("plugin_not_found"), "plugin_not_found");
    }

    #[test]
    fn generated_output_is_deterministic() {
        let text = fs::read_to_string(project_path(COMPILER_PROPERTIES)).expect("read");
        let entries = parse_entries(&text).expect("entries");
        let first = generate_accessors(&entries);
        let second = generate_accessors(&entries);
        assert_eq!(first, second);
        assert!(first.starts_with(
            "//! GENERATED by cargo xtask gen-resources from com/sun/tools/javac/resources/compiler.properties @ 982d53c"
        ));
        assert!(first.contains("pub mod errors"));
        assert!(first.contains("pub mod fragments"));
        assert!(first.contains("pub fn abstract_cant_be_instantiated(arg0: SymbolArg) -> ErrorInfo"));
        assert!(!first.contains("COMPILER_KEYS"));
        assert!(!first.contains("COMPILER_EN"));
        let metadata = generate_metadata(&entries);
        assert!(metadata.contains("pub static COMPILER_KEYS"));
        let reexport = generate_reexport();
        assert!(reexport.contains("pub use super::compiler_en::COMPILER_EN as COMPILER_MESSAGES;"));
    }

    #[test]
    fn snapshot_statistics_match_upstream() {
        let text = fs::read_to_string(project_path(COMPILER_PROPERTIES)).expect("read");
        let entries = parse_entries(&text).expect("entries");
        verify_snapshot(&entries).expect("snapshot");
        verify_names(&entries).expect("names");
    }

    #[test]
    fn scans_call_sites_with_multiline_and_nested_arguments() {
        let source = r#"class A {
    void m() {
        log(Errors.PluginNotFound(name));
        log(Errors.CantResolve(a, (b + c), new int[] { 1, 2 }));
        log(Errors
            .CantApply(
                x,
                y,
                z));
        if (Errors.SomeField == null) { }
        // Errors.InComment(a, b)
        String s = "Errors.InString(a, b)";
        log(Fragments.KindNameClass(t));
    }
}"#;
        let mut sites = Vec::new();
        scan_call_sites_in(source, "Test.java", &mut sites).expect("scan");
        let find = |name: &str| {
            sites
                .iter()
                .find(|site| site.name == name)
                .unwrap_or_else(|| panic!("missing call site {name}"))
        };
        assert_eq!(find("PluginNotFound").args, Some(1));
        assert_eq!(find("CantResolve").args, Some(3));
        assert_eq!(find("CantApply").args, Some(3));
        assert_eq!(find("SomeField").args, None);
        assert!(
            sites
                .iter()
                .all(|site| site.name != "InComment" && site.name != "InString")
        );
        assert_eq!(find("KindNameClass").args, Some(1));
        assert_eq!(find("CantApply").line, 5);
    }

    #[test]
    fn verifies_call_sites_against_generated_factories() {
        let entries = vec![Entry {
            key: "compiler.err.plugin.not.found".to_string(),
            kind: Kind::Error,
            code: "plugin.not.found".to_string(),
            value: "{0}".to_string(),
            types: vec![TypeExpr::Simple("name".to_string())],
        }];
        let ok = vec![CallSite {
            file: "A.java".to_string(),
            line: 1,
            namespace: "Errors".to_string(),
            name: "PluginNotFound".to_string(),
            args: Some(1),
        }];
        verify_call_sites(&entries, &ok).expect("matching call site");
        let bad = vec![CallSite {
            file: "A.java".to_string(),
            line: 2,
            namespace: "Errors".to_string(),
            name: "PluginNotFound".to_string(),
            args: Some(2),
        }];
        let error = verify_call_sites(&entries, &bad).expect_err("mismatch");
        assert!(error.contains("A.java:2"), "{}", error);
        assert!(error.contains("PluginNotFound"), "{}", error);
    }

    #[test]
    fn scans_reference_call_sites_without_mismatches() {
        let root = project_path(JAVAC_SOURCE_ROOT);
        if !root.is_dir() {
            eprintln!("skipping: javac reference tree not present");
            return;
        }
        let text = fs::read_to_string(project_path(COMPILER_PROPERTIES)).expect("read");
        let entries = parse_entries(&text).expect("entries");
        let sites = scan_call_sites(&root).expect("scan");
        assert!(
            sites.len() > 900,
            "expected the recorded call site volume, found {}",
            sites.len()
        );
        verify_call_sites(&entries, &sites).expect("reference call sites");
    }

    #[test]
    fn ct_and_legacy_are_not_message_bundles() {
        let root = project_path(RESOURCE_ROOT);
        if !root.is_dir() {
            eprintln!("skipping: ct/legacy properties not present");
            return;
        }
        for file in ["ct.properties", "legacy.properties"] {
            let text = fs::read_to_string(root.join(file)).expect("read");
            let map = parse_properties(&text);
            assert!(!map.is_empty(), "{file}");
            for key in map.keys() {
                assert!(
                    !key.starts_with("compiler.")
                        && !key.starts_with("launcher.")
                        && !key.starts_with("javac."),
                    "{file} has message key {key}"
                );
            }
            for locale in ["zh_CN", "ja"] {
                let translated =
                    root.join(file.replace(".properties", &format!("_{locale}.properties")));
                assert!(
                    !translated.is_file(),
                    "{file} unexpectedly has a {locale} translation"
                );
            }
        }
    }
}

