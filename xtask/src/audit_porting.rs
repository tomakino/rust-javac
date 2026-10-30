//! audit-porting: 方法级覆盖审计(PORTING_RULES §19)。
//!
//! 对 docs/PORTING_MANIFEST.md 中登记的每个上游 Java 文件:
//! 1) 输出映射状态(已映射且 Rust 文件存在 / 已映射但 Rust 文件缺失 / 未映射);
//! 2) 对已映射且存在的文件枚举上游方法/构造器声明,检查本地 Rust 文件是否出现
//!    对应函数定义(fn <snake_case>)、或在该文件有 TODO/unimplemented 登记;
//! 3) 输出缺口清单(上游方法既无本地函数也无登记)。
//!
//! 这是开发期审计工具(不是移植文件);方法声明识别是基于源码行的启发式解析,
//! 覆盖 Java 常见的修饰符/泛型/注解/数组/可变参数声明,不解析表达式上下文。
//!
//! 用法: cargo xtask audit-porting [--strict] [--max-missing N]

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const MANIFEST: &str = "docs/PORTING_MANIFEST.md";
const REFERENCE: &str = "internal/reference/jdk17u/src/jdk.compiler/share/classes";
const CORE_SRC: &str = "crates/javac-core/src";

/// 一行 manifest 记录。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManifestRow {
    pub java: String,
    pub rust: String,
    pub ported: bool,
}

/// 上游方法声明。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JavaMethod {
    pub name: String,
    pub is_ctor: bool,
    pub line: usize,
}

/// 覆盖判定。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coverage {
    /// A Rust function definition was found (exact name or an overload suffix).
    Defined,
    /// No function definition, but a TODO/unimplemented registration names it.
    Registered,
    /// The name appears in the Rust file without a definition (possible rename /
    /// macro / trait impl); requires manual review, not counted as covered.
    Mentioned,
    Missing,
}

/// 解析 docs/PORTING_MANIFEST.md 的表格行。
pub fn parse_manifest(text: &str) -> Vec<ManifestRow> {
    let mut rows = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if !line.starts_with('|') {
            continue;
        }
        let cols: Vec<&str> = line.split('|').map(str::trim).collect();
        // cols[0] is empty (leading '|'), cols[1] java, cols[2] rust, cols[3] ported.
        if cols.len() < 5 || !cols[1].ends_with(".java") {
            continue;
        }
        let java = cols[1].to_string();
        let rust = cols[2].to_string();
        let ported = cols[3] == "yes";
        rows.push(ManifestRow {
            java,
            rust,
            ported,
        });
    }
    rows
}

/// camelCase -> snake_case(与 xtask 的 Java 文件名转换保持一致的近似,但更严格:
/// 连续大写缩写只在进入小写前插入一个下划线,数字前的下划线保留)。
pub fn to_snake_case(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase() {
            let prev = i.checked_sub(1).map(|j| chars[j]);
            let next = chars.get(i + 1).copied();
            let boundary = i > 0
                && (prev.map(|p| p.is_lowercase() || p.is_ascii_digit()).unwrap_or(false)
                    || (prev.map(|p| p.is_uppercase()).unwrap_or(false)
                        && next.map(|n| n.is_lowercase()).unwrap_or(false)));
            if boundary {
                out.push('_');
            }
            for lc in c.to_lowercase() {
                out.push(lc);
            }
        } else {
            out.push(c);
        }
    }
    out
}

const MODIFIERS: &[&str] = &[
    "public", "protected", "private", "static", "final", "abstract", "synchronized",
    "native", "default", "strictfp", "transient", "volatile", "sealed", "non",
];

const NOT_METHOD_HEADS: &[&str] = &[
    "if", "for", "while", "switch", "catch", "return", "new", "else", "do", "try",
    "assert", "throw", "case", "break", "continue", "this", "super",
];

/// 去掉行尾行注释(启发式,忽略字符串内的 //)。
fn strip_line_comment(line: &str) -> &str {
    match line.find("//") {
        Some(idx) => &line[..idx],
        None => line,
    }
}

/// 去掉前导注解、修饰符与类型参数;返回剩余文本。
fn strip_head(mut text: &str) -> &str {
    loop {
        text = text.trim_start();
        if text.starts_with('@') {
            // @Anno / @Anno(...) / @Anno(a = {1, 2})
            let bytes = text.as_bytes();
            let mut i = 1;
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric()
                    || bytes[i] == b'_'
                    || bytes[i] == b'$'
                    || bytes[i] == b'.')
            {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b'(' {
                let mut depth = 0i32;
                while i < bytes.len() {
                    match bytes[i] {
                        b'(' => depth += 1,
                        b')' => {
                            depth -= 1;
                            if depth == 0 {
                                i += 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
            }
            text = &text[i..];
            continue;
        }
        let mut consumed = false;
        for kw in MODIFIERS {
            if let Some(rest) = text.strip_prefix(kw)
                && (rest.is_empty()
                    || rest.starts_with(|c: char| c.is_whitespace() || c == '(' || c == '<'))
            {
                text = rest;
                consumed = true;
                break;
            }
        }
        if consumed {
            continue;
        }
        if text.starts_with('<') {
            // 泛型方法类型参数 <T extends Foo & Bar>
            let bytes = text.as_bytes();
            let mut depth = 0i32;
            let mut i = 0;
            while i < bytes.len() {
                match bytes[i] {
                    b'<' => depth += 1,
                    b'>' => {
                        depth -= 1;
                        if depth == 0 {
                            i += 1;
                            break;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            text = &text[i..];
            continue;
        }
        return text;
    }
}

/// 从 Java 源码文本提取方法/构造器声明(启发式)。
pub fn extract_methods(java: &str, class_name: &str) -> Vec<JavaMethod> {
    let mut methods = Vec::new();
    let mut in_block_comment = false;
    for (idx, raw) in java.lines().enumerate() {
        let line_no = idx + 1;
        let mut line = raw;
        if in_block_comment {
            match line.find("*/") {
                Some(pos) => {
                    in_block_comment = false;
                    line = &line[pos + 2..];
                }
                None => continue,
            }
        }
        if let Some(pos) = line.find("/*") {
            if line[pos..].find("*/").is_none() {
                in_block_comment = true;
            }
            line = &line[..pos];
        }
        let line = strip_line_comment(line);
        let head = strip_head(line);
        if head.is_empty() || head.starts_with('{') || head.starts_with('}') {
            continue;
        }
        let Some(paren) = head.find('(') else {
            continue;
        };
        let before = head[..paren].trim_end();
        if before.is_empty() || before.ends_with('=') || before.ends_with(',') || before.ends_with('.')
        {
            continue;
        }
        if before.contains('=')
            || before.contains("->")
            || before.contains("&&")
            || before.contains("||")
            || before.contains('?')
        {
            // 表达式/初始化行,不是声明。
            continue;
        }
        let first = before.split_whitespace().next().unwrap_or("");
        if NOT_METHOD_HEADS.contains(&first) || before.contains(';') {
            continue;
        }
        let last = before.split_whitespace().last().unwrap_or("");
        let last = last.trim_end_matches(['>', ']']);
        if last.is_empty() || NOT_METHOD_HEADS.contains(&last) || last.contains('.') {
            continue;
        }
        // 构造器:名字等于类名且前无返回类型(或就是类名 token)。
        let is_ctor = last == class_name;
        // 方法名:取最后 token(可能带泛型/数组;已 trim);过滤字段声明与关键字。
        if !is_ctor {
            let tokens: Vec<&str> = before.split_whitespace().collect();
            if tokens.len() < 2 {
                continue;
            }
            // 返回类型 token 必须像类型(基本类型/大写类名/泛型/数组),否则是
            // 语句中的调用(例如 `value.foo(` 已由 '.' 排除, `x = foo(` 由 '=' 排除)。
            let type_token = tokens[tokens.len() - 2].trim_end_matches(['>', ']']);
            let looks_like_type = matches!(
                type_token,
                "void"
                    | "byte"
                    | "short"
                    | "int"
                    | "long"
                    | "float"
                    | "double"
                    | "boolean"
                    | "char"
            ) || type_token.starts_with(|c: char| c.is_uppercase());
            if !looks_like_type {
                continue;
            }
        }
        if last.starts_with(|c: char| c.is_ascii_digit()) {
            continue;
        }
        if last.contains('"') || last.contains('\'') {
            continue;
        }
        methods.push(JavaMethod {
            name: last.to_string(),
            is_ctor,
            line: line_no,
        });
    }
    methods
}

/// 判定 Rust 文件对上游方法的覆盖状态。
pub fn rust_coverage(rust: &str, java_name: &str, snake: &str) -> Coverage {
    // PORTING_RULES overload suffixes (_full/_at/_impl/...) are treated as the
    // same upstream method (registered naming convention, section 3).
    let defined = rust.contains(&format!("fn {snake}("))
        || rust.contains(&format!("fn {snake}<"))
        || rust.contains(&format!("fn {snake}_"))
        || rust.contains(&format!("fn {java_name}("))
        || rust.contains(&format!("fn {java_name}_"));
    if defined {
        return Coverage::Defined;
    }
    for line in rust.lines() {
        let is_registration = line.contains("TODO")
            || line.contains("unimplemented")
            || line.contains("NOT PORTED");
        if is_registration && (line.contains(snake) || line.contains(java_name)) {
            return Coverage::Registered;
        }
    }
    if rust.contains(snake) || rust.contains(java_name) {
        return Coverage::Mentioned;
    }
    Coverage::Missing
}

/// 单文件统计:(methods, covered, registered, mentioned, missing)。
type FileStats = (usize, usize, usize, usize, usize);

/// 审计结果。
#[derive(Default)]
pub struct AuditReport {
    pub java_files: usize,
    pub mapped_present: usize,
    pub mapped_missing: usize,
    pub unmapped: usize,
    pub methods: usize,
    pub ctors: usize,
    pub covered: usize,
    pub registered: usize,
    pub mentioned: usize,
    pub missing: usize,
    pub missing_list: Vec<(String, String, usize, String)>,
    pub mentioned_list: Vec<(String, String, usize, String)>,
    pub per_file: BTreeMap<String, FileStats>,
}

impl AuditReport {
    fn add_file(
        &mut self,
        java: &str,
        methods: usize,
        covered: usize,
        registered: usize,
        mentioned: usize,
        missing: usize,
    ) {
        self.per_file.insert(
            java.to_string(),
            (methods, covered, registered, mentioned, missing),
        );
    }
}

/// 执行审计(纯函数,便于单测):manifest 文本 + Java 源读取器 + Rust 源读取器。
pub fn audit_with<F, G>(manifest_text: &str, mut read_java: F, mut read_rust: G) -> AuditReport
where
    F: FnMut(&str) -> Option<String>,
    G: FnMut(&str) -> Option<String>,
{
    let rows = parse_manifest(manifest_text);
    let mut report = AuditReport {
        java_files: rows.len(),
        ..AuditReport::default()
    };
    for row in rows {
        if !row.ported {
            report.unmapped += 1;
            continue;
        }
        let Some(rust) = read_rust(&row.rust) else {
            report.mapped_missing += 1;
            continue;
        };
        report.mapped_present += 1;
        let Some(java) = read_java(&row.java) else {
            continue;
        };
        let class_name = Path::new(&row.java)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let methods = extract_methods(&java, &class_name);
        let previous_mentioned = report.mentioned;
        let mut covered = 0;
        let mut registered = 0;
        let mut missing = 0;
        for method in &methods {
            report.methods += 1;
            if method.is_ctor {
                report.ctors += 1;
                covered += 1;
                report.covered += 1;
                continue;
            }
            let snake = to_snake_case(&method.name);
            match rust_coverage(&rust, &method.name, &snake) {
                Coverage::Defined => {
                    covered += 1;
                    report.covered += 1;
                }
                Coverage::Registered => {
                    registered += 1;
                    report.registered += 1;
                }
                Coverage::Mentioned => {
                    report.mentioned += 1;
                    report.mentioned_list.push((
                        row.java.clone(),
                        method.name.clone(),
                        method.line,
                        row.rust.clone(),
                    ));
                }
                Coverage::Missing => {
                    missing += 1;
                    report.missing += 1;
                    report.missing_list.push((
                        row.java.clone(),
                        method.name.clone(),
                        method.line,
                        row.rust.clone(),
                    ));
                }
            }
        }
        report.add_file(
            &row.java,
            methods.len(),
            covered,
            registered,
            report.mentioned - previous_mentioned,
            missing,
        );
    }
    report
}


/// 归一化本地 fn 名:去掉 PORTING_RULES 的重载后缀/前缀适配片段
/// (_impl/_full/_at/_with_kind/_set_/_get_ ...)以及 _with_ 之后的限定部分。
pub fn normalize_fn_name(name: &str) -> String {
    let mut base = name.to_string();
    for suffix in ["_impl", "_full", "_at", "_with_kind", "_with_annotations"] {
        if let Some(stripped) = base.strip_suffix(suffix) {
            base = stripped.to_string();
        }
    }
    if let Some((head, _)) = base.split_once("_with_") {
        base = head.to_string();
    }
    base
}

/// 从 Rust 文件提取本地 fn 名(行级扫描;包含 pub/trait/inherent 定义)。
pub fn extract_rust_fns(rust: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    let mut in_test_module = false;
    for (idx, line) in rust.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("#[cfg(test)]") || trimmed.starts_with("mod tests") {
            in_test_module = true;
        }
        if in_test_module {
            continue;
        }
        let Some(pos) = trimmed.find("fn ") else {
            continue;
        };
        let rest = &trimmed[pos + 3..];
        let name: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if name.is_empty() {
            continue;
        }
        out.push((name, idx + 1));
    }
    out
}

/// 反向审计结果:本地 fn -> 官方方法。
#[derive(Debug, Default)]
pub struct ReverseReport {
    pub fns: usize,
    pub matched: Vec<String>,
    pub adapters: Vec<String>,
    pub orphans: Vec<String>,
    /// (official snake, local fn) 命名偏离候选。
    pub name_deviations: Vec<(String, String)>,
}

/// 判断 local 的标识符词是否为 official 标识符词的严格子集(命名丢词检测)。
fn word_subset(local: &str, official: &str) -> bool {
    let lw: Vec<&str> = local.split('_').filter(|s| !s.is_empty()).collect();
    let ow: Vec<&str> = official.split('_').filter(|s| !s.is_empty()).collect();
    !lw.is_empty() && lw.len() < ow.len() && lw.iter().all(|w| ow.contains(w))
}

/// 通用 trait/标准库实现名(非上游方法,归为适配实现)。
const ADAPTER_FN_NAMES: &[&str] = &[
    "new", "default", "from", "into", "deref", "deref_mut", "drop", "clone", "fmt", "eq",
    "ne", "hash", "cmp", "partial_cmp", "next", "len", "is_empty", "boxed", "to_string",
];

/// 对单个已映射文件做双向审计(reverse 部分),可单测(纯函数)。
pub fn reverse_audit(java: &str, rust: &str, class_name: &str) -> ReverseReport {
    let methods = extract_methods(java, class_name);
    let mut official: Vec<String> = methods
        .iter()
        .filter(|m| !m.is_ctor)
        .map(|m| to_snake_case(&m.name))
        .collect();
    official.sort();
    official.dedup();
    let official_set: std::collections::HashSet<&str> =
        official.iter().map(String::as_str).collect();
    let mut report = ReverseReport::default();
    for (name, line) in extract_rust_fns(rust) {
        report.fns += 1;
        let normalized = normalize_fn_name(&name);
        if official_set.contains(normalized.as_str()) {
            report.matched.push(name);
            continue;
        }
        // A local fn whose normalized words are a proper subset of an official
        // snake name indicates a naming deviation that dropped a word (e.g.
        // class_interface_body vs class_interface_or_record_body).
        let deviation = official_set
            .iter()
            .find(|off| {
                off.starts_with(&normalized) || word_subset(&normalized, off)
            })
            .map(|off| ((*off).to_string(), name.clone()));
        if let Some(dev) = deviation {
            report.name_deviations.push(dev);
            continue;
        }
        let prev_context = rust.lines().nth(line.saturating_sub(2)).unwrap_or("");
        let adapter = ADAPTER_FN_NAMES.contains(&name.as_str())
            || prev_context.contains("PORTING_RULES")
            || prev_context.contains("Java:")
            || prev_context.contains("TODO")
            || prev_context.contains("adapter");
        if adapter {
            report.adapters.push(name);
        } else {
            report.orphans.push(name);
        }
    }
    report
}

pub fn run(args: &[String]) -> Result<(), String> {
    let strict = args.iter().any(|a| a == "--strict");
    let max_missing = args
        .iter()
        .position(|a| a == "--max-missing")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(100);
    let manifest_text =
        fs::read_to_string(MANIFEST).map_err(|e| format!("read {MANIFEST}: {e}"))?;
    let reference = Path::new(REFERENCE);
    let core = Path::new(CORE_SRC);
    let rows = parse_manifest(&manifest_text);
    let report = audit_with(
        &manifest_text,
        |java| fs::read_to_string(reference.join(java)).ok(),
        |rust| fs::read_to_string(core.join(rust)).ok(),
    );
    // Reverse audit (local fn -> upstream method) for every ported file.
    let mut reverse_reports: Vec<(String, ReverseReport)> = Vec::new();
    for row in &rows {
        if !row.ported {
            continue;
        }
        let Ok(java) = fs::read_to_string(reference.join(&row.java)) else {
            continue;
        };
        let Ok(rust) = fs::read_to_string(core.join(&row.rust)) else {
            continue;
        };
        let class_name = Path::new(&row.java)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        reverse_reports.push((row.java.clone(), reverse_audit(&java, &rust, &class_name)));
    }
    // Machine-readable audit file (target/porting-audit.txt).
    let mut machine = String::new();
    machine.push_str("# porting audit (machine-readable; generated by cargo xtask audit-porting)\n");
    machine.push_str(&format!(
        "# summary java_files={} mapped_present={} mapped_missing={} unmapped={} methods={} covered={} registered={} mentioned={} missing={}\n",
        report.java_files,
        report.mapped_present,
        report.mapped_missing,
        report.unmapped,
        report.methods,
        report.covered,
        report.registered,
        report.mentioned,
        report.missing
    ));
    for (java, method, line, rust) in &report.missing_list {
        machine.push_str(&format!("POS {} {} {} {}\n", java, method, line, rust));
    }
    for (java, method, line, rust) in &report.mentioned_list {
        machine.push_str(&format!("MENTIONED {} {} {} {}\n", java, method, line, rust));
    }
    let mut orphan_total = 0usize;
    let mut adapter_total = 0usize;
    let mut deviation_total = 0usize;
    for (java, rev) in &reverse_reports {
        for orphan in &rev.orphans {
            machine.push_str(&format!("ORPHAN {} {}\n", java, orphan));
            orphan_total += 1;
        }
        for adapter in &rev.adapters {
            machine.push_str(&format!("ADAPTER {} {}\n", java, adapter));
            adapter_total += 1;
        }
        for (official, local) in &rev.name_deviations {
            machine.push_str(&format!("NAME-DEVIATION {} {} {}\n", java, official, local));
            deviation_total += 1;
        }
    }
    fs::write("target/porting-audit.txt", &machine)
        .map_err(|e| format!("write target/porting-audit.txt: {e}"))?;
    println!("== porting audit (method-level coverage, PORTING_RULES §19) ==");
    println!(
        "java files: {} (mapped+present: {}, mapped+missing: {}, unmapped: {})",
        report.java_files, report.mapped_present, report.mapped_missing, report.unmapped
    );
    println!(
        "methods: {} (constructors: {}), covered: {}, registered: {}, mentioned: {}, missing: {}",
        report.methods,
        report.ctors,
        report.covered,
        report.registered,
        report.mentioned,
        report.missing
    );
    if !report.missing_list.is_empty() {
        println!("---- missing (showing up to {} of {}) ----", max_missing, report.missing_list.len());
        for (java, method, line, rust) in report.missing_list.iter().take(max_missing) {
            println!("{java}:{line} {method} -> {rust}");
        }
    }
    // 按文件汇总有缺口的前 30 个文件。
    let mut with_missing: Vec<(&String, FileStats)> = report
        .per_file
        .iter()
        .filter(|(_, stats)| stats.4 > 0)
        .map(|(java, stats)| (java, *stats))
        .collect();
    with_missing.sort_by(|a, b| b.1.4.cmp(&a.1.4).then(a.0.cmp(b.0)));
    if !with_missing.is_empty() {
        println!("---- files with gaps (top {}) ----", with_missing.len().min(30));
        for (java, (methods, covered, registered, mentioned, missing)) in
            with_missing.iter().take(30)
        {
            println!(
                "{java}: methods {methods}, covered {covered}, registered {registered}, mentioned {mentioned}, missing {missing}"
            );
        }
    }
    println!(
        "reverse audit: local fns={}, matched={}, adapters={}, orphans={}, naming-deviations={} (target/porting-audit.txt)",
        reverse_reports.iter().map(|(_, r)| r.fns).sum::<usize>(),
        reverse_reports.iter().map(|(_, r)| r.matched.len()).sum::<usize>(),
        adapter_total,
        orphan_total,
        deviation_total
    );
    if strict && report.missing > 0 {
        return Err(format!(
            "porting audit: {} missing methods (strict mode)",
            report.missing
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_case_conversion_handles_java_names() {
        assert_eq!(to_snake_case("getThrownTypes"), "get_thrown_types");
        assert_eq!(to_snake_case("isSameType"), "is_same_type");
        assert_eq!(to_snake_case("newTypeVar"), "new_type_var");
        assert_eq!(to_snake_case("URLParser"), "url_parser");
        assert_eq!(to_snake_case("hashCode"), "hash_code");
        assert_eq!(to_snake_case("T1"), "t1");
    }

    #[test]
    fn manifest_rows_are_parsed() {
        let text = "# Porting manifest\n\n| Java source | Rust file | Ported |\n|---|---|---|\n| com/sun/tools/javac/code/Type.java | code/type.rs | yes |\n| com/sun/tools/javac/code/Flags.java | code/flags.rs |  |\n";
        let rows = parse_manifest(text);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].java, "com/sun/tools/javac/code/Type.java");
        assert_eq!(rows[0].rust, "code/type.rs");
        assert!(rows[0].ported);
        assert!(!rows[1].ported);
    }

    #[test]
    fn method_declarations_are_extracted() {
        let java = r#"
package p;
/** class doc */
public final class Sample<T> extends Object {
    private int field;
    public static void main(String... args) throws Exception { }
    @Override
    protected <U> List<U> map(Function<? super T, ? extends U> f) { return null; }
    Sample() { }
    abstract int compute(int a, int b);
    // void commentedOut() {}
    /* void blockCommentedOut() { } */
    void withLambdaBody() { if (true) { } else { } }
    String calls() {
        String s = messages.getLocalizedString("k");
        v.visitString(this);
        throw new UnsupportedOperationException();
    }
}
"#;
        let methods = extract_methods(java, "Sample");
        let names: Vec<&str> = methods.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"main"), "names: {names:?}");
        assert!(names.contains(&"map"), "names: {names:?}");
        assert!(names.contains(&"compute"), "names: {names:?}");
        assert!(names.contains(&"withLambdaBody"), "names: {names:?}");
        assert!(!names.contains(&"getLocalizedString"), "names: {names:?}");
        assert!(!names.contains(&"visitString"), "names: {names:?}");
        assert!(!names.contains(&"UnsupportedOperationException"), "names: {names:?}");
        assert!(!names.contains(&"commentedOut"), "names: {names:?}");
        assert!(!names.contains(&"blockCommentedOut"), "names: {names:?}");
        assert!(!names.contains(&"Sample<"), "ctor must be Sample: {names:?}");
        assert!(methods.iter().any(|m| m.is_ctor && m.name == "Sample"));
    }

    #[test]
    fn rust_coverage_detects_definitions_and_registrations() {
        let rust = "//! doc\nfn get_thrown_types(ctx: &CompilerContext) -> List<TypeId> { }\n// TODO(Types batch): is_capture_of not wired\nfn other() {}\n";
        assert_eq!(
            rust_coverage(rust, "getThrownTypes", "get_thrown_types"),
            Coverage::Defined
        );
        assert_eq!(
            rust_coverage(rust, "isCaptureOf", "is_capture_of"),
            Coverage::Registered
        );
        assert_eq!(
            rust_coverage(rust, "newTypeVar", "new_type_var"),
            Coverage::Missing
        );
        // Overload suffix (registered naming convention) counts as defined.
        assert_eq!(
            rust_coverage("fn get_value_impl() {}", "getValue", "get_value"),
            Coverage::Defined
        );
        // A bare mention is reported for manual review, not counted as covered.
        assert_eq!(
            rust_coverage("// v.visitString(this);", "visitString", "visit_string"),
            Coverage::Mentioned
        );
    }


    #[test]
    fn normalize_fn_name_strips_overload_suffixes() {
        assert_eq!(normalize_fn_name("class_interface_or_record_body_impl"), "class_interface_or_record_body");
        assert_eq!(normalize_fn_name("formal_parameters_full"), "formal_parameters");
        assert_eq!(normalize_fn_name("member_reference_suffix_at"), "member_reference_suffix");
        assert_eq!(normalize_fn_name("modifiers_with_annotations"), "modifiers");
        assert_eq!(normalize_fn_name("plain_name"), "plain_name");
    }

    #[test]
    fn reverse_audit_matches_adapters_and_orphans() {
        let java = "class Demo {\n    void classInterfaceOrRecordBody() { }\n    void plainMethod() { }\n}\n";
        let rust = "fn class_interface_or_record_body_impl() {}\nfn plain_method() {}\n// PORTING_RULES adapter: helper for scanner API\nfn scanner_helper() {}\nfn accidental_orphan() {}\nfn class_interface_body() {}\n";
        let report = reverse_audit(java, rust, "Demo");
        assert_eq!(report.fns, 5);
        assert_eq!(report.matched.len(), 2, "matched: {:?}", report.matched);
        assert_eq!(report.adapters.len(), 1, "adapters: {:?}", report.adapters);
        assert_eq!(report.orphans.len(), 1, "orphans: {:?}", report.orphans);
        // class_interface_body is a strict prefix of class_interface_or_record_body.
        assert!(
            report
                .name_deviations
                .iter()
                .any(|(official, local)| official == "class_interface_or_record_body"
                    && local == "class_interface_body"),
            "deviations: {:?}",
            report.name_deviations
        );
    }

    #[test]
    fn audit_counts_cover_and_missing() {
        let manifest = "| Java source | Rust file | Ported |\n|---|---|---|\n| com/sun/tools/javac/demo/A.java | demo/a.rs | yes |\n| com/sun/tools/javac/demo/B.java | demo/b.rs |  |\n";
        let java = "class A {\n    A() { }\n    void covered() { }\n    void registered() { }\n    void lost() { }\n}\n";
        let rust = "fn covered() {}\n// TODO(demo): registered is not wired yet\n";
        let report = audit_with(
            manifest,
            |_| Some(java.to_string()),
            |rust_path| {
                if rust_path == "demo/a.rs" {
                    Some(rust.to_string())
                } else {
                    None
                }
            },
        );
        assert_eq!(report.java_files, 2);
        assert_eq!(report.mapped_present, 1);
        assert_eq!(report.unmapped, 1);
        assert_eq!(report.methods, 4);
        assert_eq!(report.ctors, 1);
        assert_eq!(report.covered, 2); // ctor + covered()
        assert_eq!(report.registered, 1);
        assert_eq!(report.missing, 1);
        assert_eq!(report.missing_list[0].1, "lost");
    }
}
