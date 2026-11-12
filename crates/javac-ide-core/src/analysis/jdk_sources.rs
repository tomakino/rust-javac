//! JDK source lookup for go-to-definition.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use javac_zip::ZipArchive;

use super::positions::{byte_to_utf16, offset_to_line_col};
use super::source_files::{simple_class_name, source_relative_path};

/// A parsed JDK method's parameter names, keyed by the declaration identity
/// that matters for overload lookup. The source archive path is deliberately
/// not part of the key: the cache is for one running JDK, and a hit must not
/// re-open src.zip.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct JdkParameterKey {
    class_flat: String,
    method_name: String,
    parameter_count: usize,
    parameter_types: Vec<String>,
}

static JDK_PARAMETER_CACHE: OnceLock<Mutex<HashMap<JdkParameterKey, Vec<String>>>> =
    OnceLock::new();

#[cfg(test)]
static JDK_PARSE_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Parameter names for a JDK method/constructor, read from the JDK src.zip.
///
/// This is the last fallback after compiler symbol params and the project
/// index. The source declaration is found structurally, not by guessing a
/// random identifier in the file. The result is cached by class, method,
/// arity and parameter type names, so repeated hints do not reopen src.zip.
pub(crate) fn jdk_method_parameter_names(
    class_flat: &str,
    method_name: &str,
    parameter_count: usize,
    parameter_types: &[String],
) -> Option<Vec<String>> {
    let cache_key = JdkParameterKey {
        class_flat: class_flat.to_string(),
        method_name: method_name.to_string(),
        parameter_count,
        parameter_types: parameter_types.to_vec(),
    };
    if let Some(cached) = jdk_parameter_cache().lock().ok()?.get(&cache_key) {
        return Some(cached.clone());
    }

    let jdk_src = default_jdk_src()?;
    let text = read_class_source(&jdk_src, class_flat)?;
    #[cfg(test)]
    JDK_PARSE_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let source_name = if method_name == "<init>" {
        simple_class_name(class_flat).to_string()
    } else {
        method_name.to_string()
    };
    let allow_bare = method_name == "<init>";
    let candidates = declaration_candidates(&text, &source_name, allow_bare);
    let parsed: Vec<Vec<Parameter>> = candidates
        .into_iter()
        .filter_map(|open| parse_parameters(&text, open))
        .collect();
    let names = choose_parameter_names(parsed, parameter_count, parameter_types)?;
    jdk_parameter_cache()
        .lock()
        .ok()?
        .insert(cache_key, names.clone());
    Some(names)
}

fn jdk_parameter_cache() -> &'static Mutex<HashMap<JdkParameterKey, Vec<String>>> {
    JDK_PARAMETER_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Debug, Clone)]
struct Parameter {
    type_text: String,
    name: String,
}

fn choose_parameter_names(
    parsed: Vec<Vec<Parameter>>,
    count: usize,
    expected_types: &[String],
) -> Option<Vec<String>> {
    let mut candidates: Vec<Vec<Parameter>> = parsed
        .into_iter()
        .filter(|parameters| parameters.len() == count)
        .collect();
    if candidates.is_empty() {
        return None;
    }
    if candidates.len() > 1 {
        let expected: Vec<String> = expected_types.iter().map(|ty| normalize_type(ty)).collect();
        let scored: Vec<(usize, Vec<Parameter>)> = candidates
            .drain(..)
            .map(|parameters| {
                let score = parameters
                    .iter()
                    .zip(expected.iter())
                    .filter(|(parameter, expected)| {
                        normalize_type(&parameter.type_text) == **expected
                    })
                    .count();
                (score, parameters)
            })
            .collect();
        let best = scored.iter().map(|(score, _)| *score).max().unwrap_or(0);
        if best > 0 {
            let winners: Vec<&(usize, Vec<Parameter>)> = scored
                .iter()
                .filter(|(score, _)| *score == best)
                .collect();
            if winners.len() != 1 {
                return None;
            }
            candidates.push(winners[0].1.clone());
        } else {
            // The descriptor is not usable for a generic type variable. If
            // every same-arity declaration exposes the same names, they are
            // unambiguous and safe to use.
            let first: Vec<String> = scored[0].1.iter().map(|p| p.name.clone()).collect();
            if scored
                .iter()
                .all(|(_, parameters)| parameters.iter().map(|p| &p.name).eq(first.iter()))
            {
                return Some(first);
            }
            return None;
        }
    }
    let parameters = candidates.pop()?;
    Some(parameters.into_iter().map(|parameter| parameter.name).collect())
}

fn read_class_source(jdk_src: &Path, class_flat: &str) -> Option<String> {
    if !jdk_src.exists() {
        return None;
    }
    let relative = source_relative_path(class_flat)?;
    let archive = ZipArchive::open(jdk_src).ok()?;
    let suffix = format!("/{relative}");
    let entry_name = archive
        .entries()
        .iter()
        .map(|entry| entry.name().to_string())
        .find(|name| name == &relative || name.ends_with(&suffix))?;
    let bytes = archive.get_entry(&entry_name)?;
    Some(String::from_utf8_lossy(&bytes).to_string())
}

/// Byte offsets of `(` belonging to structurally plausible declarations of
/// `name`. Calls, assignments, `new` expressions, comments and strings are
/// rejected. This is a declaration locator, not a search for arbitrary text.
fn declaration_candidates(text: &str, name: &str, allow_bare: bool) -> Vec<usize> {
    let mask = code_mask(text);
    let mut candidates = Vec::new();
    let mut cursor = 0usize;
    while let Some(relative) = text[cursor..].find(name) {
        let start = cursor + relative;
        let end = start + name.len();
        cursor = end;
        if start > 0
            && text[..start]
                .chars()
                .next_back()
                .is_some_and(is_identifier_char)
        {
            continue;
        }
        if text[end..]
            .chars()
            .next()
            .is_some_and(is_identifier_char)
        {
            continue;
        }
        if !mask.get(start).copied().unwrap_or(false) {
            continue;
        }
        let Some(open) = next_non_whitespace(text, end) else {
            continue;
        };
        if !text.get(open..).is_some_and(|rest| rest.starts_with('(')) {
            continue;
        }
        let line_start = text[..start].rfind('\n').map_or(0, |index| index + 1);
        let prefix = &text[line_start..start];
        let trimmed = prefix.trim_start();
        let previous = text[..start].chars().next_back();
        if previous == Some('.') || prefix.contains('=') {
            continue;
        }
        if trimmed.is_empty() && !allow_bare {
            continue;
        }
        if trimmed.starts_with("//")
            || trimmed.starts_with('*')
            || trimmed.starts_with("/*")
            || trimmed.starts_with("return")
            || trimmed.starts_with("throw")
            || trimmed.starts_with("new")
            || trimmed.starts_with("else")
            || trimmed.starts_with("case")
            || trimmed.starts_with("if")
            || trimmed.starts_with("for")
            || trimmed.starts_with("while")
            || trimmed.starts_with("switch")
            || trimmed.starts_with("catch")
            || trimmed.starts_with("assert")
            || trimmed.starts_with("yield")
        {
            continue;
        }
        if !allow_bare && trimmed.is_empty() {
            continue;
        }
        candidates.push(open);
    }
    candidates
}

fn next_non_whitespace(text: &str, mut position: usize) -> Option<usize> {
    while position < text.len() {
        if !text[position..].chars().next()?.is_whitespace() {
            return Some(position);
        }
        position += text[position..].chars().next()?.len_utf8();
    }
    None
}

fn parse_parameters(text: &str, open: usize) -> Option<Vec<Parameter>> {
    let mask = code_mask(text);
    let close = matching_paren(text, open, &mask)?;
    let body = &text[open + 1..close];
    if body.trim().is_empty() {
        return Some(Vec::new());
    }
    let mut parameters = Vec::new();
    for segment in split_parameters(body, &mask[open + 1..close]) {
        if let Some(parameter) = parse_parameter(segment) {
            parameters.push(parameter);
        } else {
            return None;
        }
    }
    Some(parameters)
}

fn split_parameters<'a>(body: &'a str, mask: &[bool]) -> Vec<&'a str> {
    let mut result = Vec::new();
    let mut start = 0usize;
    let mut parens = 0usize;
    let mut angles = 0usize;
    let mut brackets = 0usize;
    let mut braces = 0usize;
    for (index, ch) in body.char_indices() {
        if !mask.get(index).copied().unwrap_or(true) {
            continue;
        }
        match ch {
            '(' => parens += 1,
            ')' => parens = parens.saturating_sub(1),
            '<' => angles += 1,
            '>' => angles = angles.saturating_sub(1),
            '[' => brackets += 1,
            ']' => brackets = brackets.saturating_sub(1),
            '{' => braces += 1,
            '}' => braces = braces.saturating_sub(1),
            ',' if parens == 0 && angles == 0 && brackets == 0 && braces == 0 => {
                result.push(&body[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    result.push(&body[start..]);
    result
}

fn parse_parameter(segment: &str) -> Option<Parameter> {
    let text = strip_annotations(segment);
    let text = cut_default_value(&text);
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let mask = code_mask(text);
    let mut name_start = None;
    let mut name_end = 0usize;
    let mut index = 0usize;
    while index < text.len() {
        let ch = text[index..].chars().next()?;
        if is_identifier_start(ch) && mask.get(index).copied().unwrap_or(false) {
            let end = index + ch.len_utf8();
            let mut end = end;
            while end < text.len() {
                let next = text[end..].chars().next()?;
                if is_identifier_char(next) {
                    end += next.len_utf8();
                } else {
                    break;
                }
            }
            name_start = Some(index);
            name_end = end;
            index = end;
        } else {
            index += ch.len_utf8();
        }
    }
    let name_start = name_start?;
    let name = text[name_start..name_end].to_string();
    let type_text = text[..name_start].trim().to_string();
    if type_text.is_empty() || name.is_empty() {
        return None;
    }
    Some(Parameter { type_text, name })
}

fn strip_annotations(text: &str) -> String {
    let mut output = String::new();
    let mut index = 0usize;
    while index < text.len() {
        let ch = text[index..].chars().next().unwrap();
        if ch != '@' {
            output.push(ch);
            index += ch.len_utf8();
            continue;
        }
        index += 1;
        while index < text.len() {
            let ch = text[index..].chars().next().unwrap();
            if is_identifier_char(ch) || ch == '.' {
                index += ch.len_utf8();
            } else {
                break;
            }
        }
        if text[index..].trim_start().starts_with('(') {
            if let Some(open) = next_non_whitespace(text, index) {
                let mask = code_mask(text);
                if let Some(close) = matching_paren(text, open, &mask) {
                    index = close + 1;
                }
            }
        }
        output.push(' ');
    }
    output
}

fn cut_default_value(text: &str) -> String {
    let mask = code_mask(text);
    let mut parens = 0usize;
    let mut angles = 0usize;
    for (index, ch) in text.char_indices() {
        if !mask.get(index).copied().unwrap_or(true) {
            continue;
        }
        match ch {
            '(' => parens += 1,
            ')' => parens = parens.saturating_sub(1),
            '<' => angles += 1,
            '>' => angles = angles.saturating_sub(1),
            '=' if parens == 0 && angles == 0 => return text[..index].to_string(),
            _ => {}
        }
    }
    text.to_string()
}

fn matching_paren(text: &str, open: usize, mask: &[bool]) -> Option<usize> {
    if !text.get(open..).is_some_and(|rest| rest.starts_with('(')) {
        return None;
    }
    let mut depth = 0usize;
    for (index, ch) in text[open..].char_indices() {
        let index = open + index;
        if !mask.get(index).copied().unwrap_or(false) {
            continue;
        }
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

/// Mark bytes that are Java code (not comments or literal strings).
fn code_mask(text: &str) -> Vec<bool> {
    let bytes = text.as_bytes();
    let mut mask = vec![true; bytes.len()];
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'/') {
            let start = index;
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            mask[start..index].fill(false);
        } else if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'*') {
            let start = index;
            index += 2;
            while index + 1 < bytes.len() && !(bytes[index] == b'*' && bytes[index + 1] == b'/') {
                index += 1;
            }
            index = (index + 2).min(bytes.len());
            mask[start..index].fill(false);
        } else if bytes[index] == b'"' {
            let start = index;
            index += 1;
            while index < bytes.len() {
                if bytes[index] == b'\\' {
                    index += 2;
                } else if bytes[index] == b'"' {
                    index += 1;
                    break;
                } else {
                    index += 1;
                }
            }
            mask[start..index.min(bytes.len())].fill(false);
        } else if bytes[index] == b'\'' {
            let start = index;
            index += 1;
            while index < bytes.len() {
                if bytes[index] == b'\\' {
                    index += 2;
                } else if bytes[index] == b'\'' {
                    index += 1;
                    break;
                } else {
                    index += 1;
                }
            }
            mask[start..index.min(bytes.len())].fill(false);
        } else {
            index += 1;
        }
    }
    mask
}

fn normalize_type(text: &str) -> String {
    let text = strip_annotations(text);
    let text = text.trim();
    let text = text
        .strip_suffix("...")
        .or_else(|| text.strip_suffix("[]"))
        .unwrap_or(text)
        .trim();
    let mut depth = 0usize;
    let mut base = String::new();
    for ch in text.chars() {
        match ch {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            ch if depth == 0 => base.push(ch),
            _ => {}
        }
    }
    let base = base.trim();
    base.rsplit('.').next().unwrap_or(base).to_string()
}

fn is_identifier_start(ch: char) -> bool {
    ch.is_alphabetic() || ch == '_' || ch == '$'
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_' || ch == '$'
}

/// Find the JDK source declaration of a simple name in the JDK src.zip.
///
/// The matching entry is extracted under the session output directory so the
/// editor can open it as a normal file.
pub(crate) fn jdk_source_location(
    jdk_src: &Path,
    word: &str,
    output_dir: &Path,
) -> Option<javac_analysis_api::Location> {
    if !jdk_src.exists() {
        return None;
    }
    let archive = ZipArchive::open(jdk_src).ok()?;
    let suffix = format!("/{word}.java");
    let entry_name = archive
        .entries()
        .iter()
        .map(|entry| entry.name().to_string())
        .find(|name| name.ends_with(&suffix) || name == &format!("{word}.java"))?;
    let bytes = archive.get_entry(&entry_name)?;
    let text = String::from_utf8_lossy(&bytes).to_string();
    let markers = [
        format!("class {word}"),
        format!("interface {word}"),
        format!("enum {word}"),
        format!("record {word}"),
    ];
    let offset = text
        .lines()
        .enumerate()
        .find(|(_, line)| markers.iter().any(|marker| line.contains(marker.as_str())))
        .map(|(index, _)| {
            text.lines()
                .take(index)
                .map(|line| line.len() + 1)
                .sum::<usize>()
        })?;
    let extracted = super::source_files::extract(&bytes, "jdk-src", &entry_name, output_dir)?;
    Some(location_at_offset(&text, &extracted, offset))
}

/// Find a member declaration in the JDK src.zip for `class_flat`.
///
/// JDK 9+ src.zip entries are prefixed with the module name
/// (`java.base/java/lang/Object.java`), so match the class path as a suffix.
pub(crate) fn jdk_member_location(
    jdk_src: &Path,
    class_flat: &str,
    member: &str,
    output_dir: &Path,
) -> Option<javac_analysis_api::Location> {
    let (text, extracted) = class_source_text(jdk_src, class_flat, output_dir)?;
    let offset = super::external_sources::locate_member(&text, member)?;
    Some(location_at_offset(&text, &extracted, offset))
}

/// Find a constructor declaration in the JDK src.zip for `class_flat`.
///
/// `arity` selects the overload by declared parameter count; the caller
/// degrades to [`jdk_class_location`] when the source text cannot pick a
/// unique declaration.
pub(crate) fn jdk_constructor_location(
    jdk_src: &Path,
    class_flat: &str,
    arity: usize,
    output_dir: &Path,
) -> Option<javac_analysis_api::Location> {
    let (text, extracted) = class_source_text(jdk_src, class_flat, output_dir)?;
    let simple = simple_class_name(class_flat);
    let offset = super::external_sources::locate_constructor(&text, simple, Some(arity))?;
    Some(location_at_offset(&text, &extracted, offset))
}

/// Find the type declaration of `class_flat` in the JDK src.zip.
pub(crate) fn jdk_class_location(
    jdk_src: &Path,
    class_flat: &str,
    output_dir: &Path,
) -> Option<javac_analysis_api::Location> {
    let (text, extracted) = class_source_text(jdk_src, class_flat, output_dir)?;
    let simple = simple_class_name(class_flat);
    // `locate_member` prefers the type declaration (`class X`) over members.
    let offset = super::external_sources::locate_member(&text, simple)?;
    Some(location_at_offset(&text, &extracted, offset))
}

/// Read `class_flat`'s source from the JDK src.zip and extract it under the
/// session output directory. Returns the source text and the extracted path.
fn class_source_text(
    jdk_src: &Path,
    class_flat: &str,
    output_dir: &Path,
) -> Option<(String, PathBuf)> {
    if !jdk_src.exists() {
        return None;
    }
    let relative = source_relative_path(class_flat)?;
    let archive = ZipArchive::open(jdk_src).ok()?;
    let suffix = format!("/{relative}");
    let entry_name = archive
        .entries()
        .iter()
        .map(|entry| entry.name().to_string())
        .find(|name| name == &relative || name.ends_with(&suffix))?;
    let bytes = archive.get_entry(&entry_name)?;
    let text = String::from_utf8_lossy(&bytes).to_string();
    let extracted = super::source_files::extract(&bytes, "jdk-src", &entry_name, output_dir)?;
    Some((text, extracted))
}

/// LSP location of a byte offset in an extracted JDK source file.
///
/// The byte offset becomes a UTF-16 offset before the line/column conversion,
/// so non-ASCII source lines keep the editor cursor on the identifier.
fn location_at_offset(
    text: &str,
    file: &Path,
    offset: usize,
) -> javac_analysis_api::Location {
    let offset = byte_to_utf16(text, offset);
    let (line, column) = offset_to_line_col(text, offset);
    javac_analysis_api::Location {
        file: file.to_path_buf(),
        range: javac_analysis_api::Range::point(javac_analysis_api::Position::new(line, column)),
    }
}

/// The default JDK src.zip location. Delegates to the single locator in
/// [`super::source_files`] (union of RAVAC_PLATFORM_DIR / RAVAC_JDK_SRC /
/// JAVA_HOME); no second locator is kept here.
pub(crate) fn default_jdk_src() -> Option<PathBuf> {
    super::source_files::jdk_src_zip()
}

/// Major JDK version from the platform/`JAVA_HOME` `release` file, when
/// available. `None` means the source annotation omits the version rather
/// than guessing it from a path.
pub(crate) fn jdk_release_version() -> Option<String> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(platform) = std::env::var_os("RAVAC_PLATFORM_DIR") {
        roots.push(PathBuf::from(platform));
    }
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        roots.push(PathBuf::from(home));
    }
    if let Some(src) = default_jdk_src() {
        if let Some(parent) = src.parent() {
            roots.push(parent.to_path_buf());
            if parent.file_name().is_some_and(|name| name == "lib")
                && let Some(home) = parent.parent()
            {
                roots.push(home.to_path_buf());
            }
        }
    }
    for root in roots {
        let Ok(text) = std::fs::read_to_string(root.join("release")) else {
            continue;
        };
        if let Some(value) = text
            .lines()
            .find_map(|line| line.strip_prefix("JAVA_VERSION="))
        {
            let value = value.trim().trim_matches('"');
            let major = value.split('.').next().unwrap_or(value);
            if !major.is_empty() {
                return Some(major.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod parameter_tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn jdk_parameter_names_cache_hit_does_not_reparse() {
        if default_jdk_src().is_none() {
            return;
        }
        if let Ok(mut cache) = jdk_parameter_cache().lock() {
            cache.clear();
        }
        let before = JDK_PARSE_COUNT.load(Ordering::SeqCst);
        let first = jdk_method_parameter_names("java.util.List", "add", 1, &["E".to_string()])
            .expect("List.add parameter names");
        assert_eq!(first, vec!["e".to_string()]);
        let after_first = JDK_PARSE_COUNT.load(Ordering::SeqCst);
        assert_eq!(after_first, before + 1, "first lookup must parse once");

        let second = jdk_method_parameter_names("java.util.List", "add", 1, &["E".to_string()])
            .expect("cached List.add parameter names");
        assert_eq!(second, first);
        assert_eq!(
            JDK_PARSE_COUNT.load(Ordering::SeqCst),
            after_first,
            "cache hit must not parse src.zip again"
        );
    }
}
