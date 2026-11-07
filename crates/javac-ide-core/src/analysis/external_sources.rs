//! Classpath source lookup for go-to-definition.
//!
//! A definition may point at a class from a jar. When the matching
//! `-sources.jar` is available next to the binary jar, the source file is
//! extracted under the session output directory so the editor can open it.

use std::path::{Path, PathBuf};

use javac_zip::ZipArchive;

use super::positions::{byte_to_utf16, offset_to_line_col};
use super::source_files::{simple_class_name, source_relative_path, zip_entry};

/// Locate `member` in the sources jar for `class_flat` on the class path.
pub(crate) fn external_source_location(
    class_flat: &str,
    member: &str,
    class_path: &[PathBuf],
    output_dir: &Path,
) -> Option<javac_analysis_api::Location> {
    sources_location(
        class_flat,
        &|text, _class| locate_member(text, member),
        class_path,
        output_dir,
    )
}

/// Locate a constructor declaration of `class_flat` in its sources jar.
///
/// `arity` distinguishes overloads. The declaration is matched in the source
/// text (no compiler tree exists for class-path classes); when no unique
/// declaration matches, the caller degrades to the class declaration.
pub(crate) fn external_constructor_location(
    class_flat: &str,
    arity: usize,
    class_path: &[PathBuf],
    output_dir: &Path,
) -> Option<javac_analysis_api::Location> {
    sources_location(
        class_flat,
        &|text, class_name| locate_constructor(text, class_name, Some(arity)),
        class_path,
        output_dir,
    )
}

/// Shared `-sources.jar` lookup.
///
/// `locate(text, simple class name)` returns the byte offset of the
/// declaration name inside the extracted source file.
fn sources_location(
    class_flat: &str,
    locate: &dyn Fn(&str, &str) -> Option<usize>,
    class_path: &[PathBuf],
    output_dir: &Path,
) -> Option<javac_analysis_api::Location> {
    let relative = source_relative_path(class_flat)?;
    let simple = simple_class_name(class_flat);
    for entry in class_path {
        let Some(sources) = super::source_files::sources_jar(entry) else {
            continue;
        };
        let Ok(archive) = ZipArchive::open(&sources) else {
            continue;
        };
        let Some(entry_name) = zip_entry(&archive, &relative) else {
            continue;
        };
        let Some(bytes) = archive.get_entry(&entry_name) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes).to_string();
        let Some(offset) = locate(&text, simple) else {
            continue;
        };
        let stem = sources.file_stem()?.to_string_lossy();
        let extracted = super::source_files::extract(
            &bytes,
            &format!("external-src/{stem}"),
            &relative,
            output_dir,
        )?;
        let offset = byte_to_utf16(&text, offset);
        let (line, column) = offset_to_line_col(&text, offset);
        return Some(javac_analysis_api::Location {
            file: PathBuf::from(extracted),
            range: javac_analysis_api::Range::point(javac_analysis_api::Position::new(
                line, column,
            )),
        });
    }
    None
}

/// Byte offset of the declaration name of `member` in `text`.
///
/// The returned offset points at the name itself (not the line start), so the
/// editor cursor lands on the identifier. Type declarations win over members;
/// member calls are skipped (they are preceded by a dot or start the line as a
/// bare call).
pub(crate) fn locate_member(text: &str, member: &str) -> Option<usize> {
    if member.is_empty() {
        return None;
    }
    let mut offset = 0usize;
    for line in text.lines() {
        if let Some(column) =
            type_declaration_column(line, member).or_else(|| member_declaration_column(line, member))
        {
            return Some(offset + column);
        }
        offset += line.len() + 1;
    }
    None
}

/// Byte offset of a constructor declaration name of `class_name` in `text`.
///
/// Last-resort text locator for external sources (the compiler tree only
/// exists for project sources). A declaration is a line that starts with
/// `public`/`protected`/`private` or with the bare class name (package-private
/// constructor); a call (`new X(...)`, `x.X(...)`), a comment and the
/// `record X(...)` header are rejected. `arity` selects an overload by its
/// declared parameter count; a `None` arity yields the first declaration.
/// More than one declaration with the requested arity is ambiguous and
/// returns `None` (the caller falls back to the class declaration).
pub(crate) fn locate_constructor(
    text: &str,
    class_name: &str,
    arity: Option<usize>,
) -> Option<usize> {
    if class_name.is_empty() {
        return None;
    }
    let mut offset = 0usize;
    let mut matches = Vec::new();
    for line in text.lines() {
        if let Some(column) = constructor_name_column(line, class_name) {
            let name = offset + column;
            let after = &line[column + class_name.len()..];
            // The declaration form allows whitespace before the parameter
            // list (`A ()`); the list itself may span lines.
            let gap = after.len() - after.trim_start().len();
            let open = name + class_name.len() + gap;
            if let Some(count) = parameter_count(text, open) {
                if arity == Some(count) {
                    matches.push(name);
                } else if arity.is_none() && matches.is_empty() {
                    matches.push(name);
                }
            }
        }
        offset += line.len() + 1;
    }
    match arity {
        Some(_) => (matches.len() == 1).then(|| matches[0]),
        None => matches.first().copied(),
    }
}

/// Byte column of the declared class name, when `line` declares a constructor
/// of `class_name`.
fn constructor_name_column(line: &str, class_name: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") || trimmed.starts_with('*') || trimmed.starts_with("/*") {
        return None;
    }
    // Declaration form: a modifier starts the line, or the class name itself
    // starts it (a package-private constructor).
    let declared = ["public", "protected", "private"]
        .iter()
        .any(|modifier| starts_with_word(trimmed, modifier))
        || trimmed.starts_with(&format!("{class_name}("))
        || trimmed.starts_with(&format!("{class_name} ("));
    if !declared {
        return None;
    }
    let mut start = line.len() - trimmed.len();
    while let Some(position) = line[start..].find(class_name) {
        let absolute = start + position;
        let before = &line[..absolute];
        let after = &line[absolute + class_name.len()..];
        // Reject `new X(`, `x.X(`, a preceding type declaration keyword
        // (`record X(...)`) and anything that is not a plain declaration
        // prefix (expressions, statements, comments).
        let before_ok = !before.trim_end().ends_with('.')
            && !ends_with_word(before, "new")
            && !ends_with_word(before, "record")
            && !ends_with_word(before, "class")
            && !ends_with_word(before, "interface")
            && !ends_with_word(before, "enum")
            && !before.contains(['(', ')', '{', '}', ';', '=', '/', ',']);
        let after_ok = after.trim_start().starts_with('(');
        if before_ok && after_ok {
            return Some(absolute);
        }
        start = absolute + class_name.len();
    }
    None
}

/// Number of parameters in the parenthesized list at byte `open` (`text[open]`
/// is the `(`), or `None` when the list is unterminated.
///
/// Commas nested in another parenthesis, in generic type arguments or in an
/// annotation's braces do not separate parameters. Strings are skipped so a
/// quoted comma cannot inflate the count.
fn parameter_count(text: &str, open: usize) -> Option<usize> {
    let tail = text.get(open..)?;
    if !tail.starts_with('(') {
        return None;
    }
    let mut parens = 0usize;
    let mut angles = 0usize;
    let mut braces = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut content = false;
    let mut count = 1usize;
    for ch in tail.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => {
                in_string = true;
                content = true;
            }
            '(' => parens += 1,
            ')' => {
                parens = parens.saturating_sub(1);
                if parens == 0 {
                    return Some(if content { count } else { 0 });
                }
            }
            '<' => {
                angles += 1;
                content = true;
            }
            '>' => angles = angles.saturating_sub(1),
            '{' => {
                braces += 1;
                content = true;
            }
            '}' => braces = braces.saturating_sub(1),
            ',' if parens == 1 && angles == 0 && braces == 0 => count += 1,
            ch if !ch.is_whitespace() => content = true,
            _ => {}
        }
    }
    None
}

/// Whether `text` starts with `word` followed by whitespace.
fn starts_with_word(text: &str, word: &str) -> bool {
    text.strip_prefix(word)
        .is_some_and(|rest| rest.chars().next().is_none_or(char::is_whitespace))
}

/// Whether `text` ends with `word` at a word boundary.
fn ends_with_word(text: &str, word: &str) -> bool {
    let trimmed = text.trim_end();
    trimmed.strip_suffix(word).is_some_and(|before| {
        before
            .chars()
            .next_back()
            .is_none_or(|ch| !(ch.is_alphanumeric() || ch == '_' || ch == '$'))
    })
}

/// Byte column of the declared type name, when `line` declares `member`.
fn type_declaration_column(line: &str, member: &str) -> Option<usize> {
    // Javadoc prose mentions "a class Foo" too; never treat comments as
    // declarations.
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") || trimmed.starts_with('*') || trimmed.starts_with("/*") {
        return None;
    }
    for keyword in ["class", "interface", "enum", "record"] {
        let marker = format!("{keyword} {member}");
        if let Some(position) = line.find(&marker) {
            let rest = &line[position + marker.len()..];
            if rest.is_empty()
                || rest.starts_with(' ')
                || rest.starts_with('{')
                || rest.starts_with('<')
                || rest.starts_with('(')
            {
                return Some(position + keyword.len() + 1);
            }
        }
    }
    None
}

/// Byte column of the declared member name, when `line` declares `member`.
fn member_declaration_column(line: &str, member: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("//")
        || trimmed.starts_with('*')
        || trimmed.starts_with("return")
        || trimmed.starts_with("throw")
        || trimmed.starts_with("package")
        || trimmed.starts_with("import")
    {
        return None;
    }
    let mut start = 0;
    while let Some(position) = line[start..].find(member) {
        let absolute = start + position;
        let before = line[..absolute].trim_end();
        let after = line[absolute + member.len()..].trim_start();
        let before_ok = !before.is_empty() && !before.ends_with('.') && !before.contains('=');
        let after_ok =
            after.starts_with('(') || after.starts_with('=') || after.starts_with(';');
        if before_ok && after_ok {
            return Some(absolute);
        }
        start = absolute + member.len();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_method_declaration_before_the_call() {
        let text = "public class StringUtils {\n    public static String join(Object... elements) {\n        return join(elements);\n    }\n}\n";
        let offset = locate_member(text, "join").expect("join");
        assert_eq!(
            offset,
            "public class StringUtils {\n    public static String ".len(),
            "cursor must land on the declared name"
        );
    }

    #[test]
    fn finds_a_type_declaration() {
        let text = "package com.example;\n\npublic class Service {\n}\n";
        let offset = locate_member(text, "Service").expect("Service");
        assert_eq!(offset, "package com.example;\n\npublic class ".len());
    }

    #[test]
    fn skips_member_calls() {
        let text = "class A {\n    void f() {\n        StringUtils.join(1);\n    }\n}\n";
        assert!(locate_member(text, "join").is_none());
    }

    #[test]
    fn sources_jar_is_next_to_the_binary_jar() {
        let root = PathBuf::from("target/tmp/external-sources-spec");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let jar = root.join("lib-1.0.jar");
        let sources = root.join("lib-1.0-sources.jar");
        std::fs::write(&jar, b"").unwrap();
        std::fs::write(&sources, b"").unwrap();
        assert_eq!(super::super::source_files::sources_jar(&jar), Some(sources));
        assert_eq!(super::super::source_files::sources_jar(&root.join("other.jar")), None);
    }

    #[test]
    fn constructor_arity_selects_the_overload() {
        let text = "public class RuntimeException extends Exception {\n    public RuntimeException() {\n        super();\n    }\n\n    public RuntimeException(String message) {\n        super(message);\n    }\n}\n";
        let no_arg = locate_constructor(text, "RuntimeException", Some(0)).expect("no-arg");
        assert_eq!(
            no_arg,
            "public class RuntimeException extends Exception {\n    public ".len()
        );
        let message = locate_constructor(text, "RuntimeException", Some(1)).expect("String");
        assert!(text[message..].starts_with("RuntimeException(String message)"));
    }

    #[test]
    fn constructor_parameters_may_span_lines() {
        let text = "class E extends Exception {\n    protected E(String message, Throwable cause,\n                boolean enableSuppression) {\n        super();\n    }\n}\n";
        assert!(
            locate_constructor(text, "E", Some(3)).is_some(),
            "multi-line parameter list"
        );
        assert!(locate_constructor(text, "E", Some(2)).is_none());
    }

    #[test]
    fn skips_calls_comments_and_record_headers() {
        let text = "public record Point(int x, int y) {\n    public Point {\n    }\n    void f() {\n        new Point(1, 2);\n        factory.Point(1, 2);\n    }\n}\n";
        assert!(locate_constructor(text, "Point", Some(2)).is_none());
        let commented = "class A {\n    // A() {}\n    public static void f() { /* A() */ }\n}\n";
        assert!(locate_constructor(commented, "A", Some(0)).is_none());
    }

    #[test]
    fn package_private_constructor_is_a_declaration() {
        let text = "class E {\n    E() {\n    }\n}\n";
        assert_eq!(
            locate_constructor(text, "E", Some(0)),
            Some("class E {\n    ".len())
        );
    }

    #[test]
    fn ambiguous_same_arity_returns_none() {
        let text = "class A {\n    public A(int x) {}\n    public A(String s) {}\n}\n";
        assert!(
            locate_constructor(text, "A", Some(1)).is_none(),
            "same-arity overloads cannot be told apart by arity"
        );
    }

    #[test]
    fn generic_parameter_types_do_not_add_parameters() {
        let text = "class A {\n    public A(java.util.Map<String, Integer> m) {}\n}\n";
        assert!(
            locate_constructor(text, "A", Some(1)).is_some(),
            "the comma inside the type arguments is not a separator"
        );
    }
}

#[cfg(test)]
mod javadoc_tests {
    use super::*;

    #[test]
    fn javadoc_prose_is_not_a_declaration() {
        let text = " * It is an error to declare a class Serializable if this is not\n * the case.\n */\npublic interface Serializable {\n}\n";
        let offset = locate_member(text, "Serializable").expect("declaration");
        assert_eq!(offset, " * It is an error to declare a class Serializable if this is not\n * the case.\n */\npublic interface ".len());
    }
}
