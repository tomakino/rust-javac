//! Classpath source lookup for go-to-definition.
//!
//! A definition may point at a class from a jar. When the matching
//! `-sources.jar` is available next to the binary jar, the source file is
//! extracted under the session output directory so the editor can open it.

use std::path::{Path, PathBuf};

use javac_zip::ZipArchive;

use super::positions::{byte_to_utf16, offset_to_line_col};

/// Locate `member` in the sources jar for `class_flat` on the class path.
pub(crate) fn external_source_location(
    class_flat: &str,
    member: &str,
    class_path: &[PathBuf],
    output_dir: &Path,
) -> Option<javac_analysis_api::Location> {
    // Nested classes live in the outer class's source file.
    let class_name = class_flat.split('$').next().unwrap_or(class_flat);
    if class_name.is_empty() {
        return None;
    }
    let relative = format!("{}.java", class_name.replace('.', "/"));
    for entry in class_path {
        let Some(sources) = super::source_files::sources_jar(entry) else {
            continue;
        };
        let Ok(archive) = ZipArchive::open(&sources) else {
            continue;
        };
        let Some(entry_name) = super::source_files::zip_entry(&archive, &relative) else {
            continue;
        };
        let Some(bytes) = archive.get_entry(&entry_name) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes).to_string();
        let Some(offset) = locate_member(&text, member) else {
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
