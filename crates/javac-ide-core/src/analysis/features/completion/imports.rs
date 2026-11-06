//! Auto-import edits for completion.
//!
//! IDEA adds the import statement when a completion inserts a type that is not
//! visible yet. The edit is attached to the completion item as an LSP
//! additionalTextEdits entry; the client applies both edits atomically.

use javac_analysis_api::{Position, Range, TextEdit};

/// The import-relevant state of the file being completed.
pub(crate) struct ImportContext {
    package: String,
    /// Explicit non-static single-type imports (dotted names).
    imports: Vec<String>,
    /// Packages imported with a wildcard.
    wildcard: Vec<String>,
    /// Types declared in the current file (do not import over them).
    local_names: Vec<String>,
    /// Zero-length insertion point after the package/import block.
    insertion: Position,
    /// True when the insertion point is not preceded by a line break.
    needs_leading_newline: bool,
    /// False for import-declaration completion.
    enabled: bool,
}

impl ImportContext {
    pub(crate) fn new(text: &str, local_names: Vec<String>, enabled: bool) -> Self {
        let mut package = String::new();
        let mut imports = Vec::new();
        let mut wildcard = Vec::new();
        let mut insert_at = 0usize;
        let mut offset = 0usize;
        for line in text.split_inclusive('\n') {
            let end = offset + line.len();
            // Tolerate a trailing line comment on a package/import line.
            let code = line
                .split("//")
                .next()
                .unwrap_or(line)
                .trim();
            if let Some(rest) = code.strip_prefix("package ") {
                if let Some(name) = rest.strip_suffix(';') {
                    package = name.trim().to_string();
                    insert_at = end;
                }
            } else if let Some(rest) = code.strip_prefix("import ") {
                if let Some(name) = rest.strip_suffix(';') {
                    let name = name.trim();
                    if let Some(package_name) = name.strip_suffix(".*") {
                        wildcard.push(package_name.to_string());
                    } else if !name.starts_with("static ") {
                        imports.push(name.to_string());
                    }
                    insert_at = end;
                }
            }
            offset = end;
        }
        // `insert_at` is a byte offset; LSP positions use UTF-16 columns.
        let insert_utf16 = crate::analysis::positions::byte_to_utf16(text, insert_at);
        let (line, column) = crate::analysis::positions::offset_to_line_col(text, insert_utf16);
        Self {
            package,
            imports,
            wildcard,
            local_names,
            insertion: Position::new(line, column),
            needs_leading_newline: insert_at > 0 && !text[..insert_at].ends_with('\n'),
            enabled,
        }
    }

    /// The import edit needed to reference `flat`, or None when the type is
    /// already visible/imported or auto-import is not applicable.
    pub(crate) fn edit_for(&self, flat: &str, is_public: bool) -> Option<TextEdit> {
        if !self.enabled || !is_public {
            return None;
        }
        let (package, simple) = flat.rsplit_once('.')?;
        if package.is_empty() || simple.is_empty() || simple.contains('$') {
            return None;
        }
        if package == self.package || package == "java.lang" {
            return None;
        }
        if self.imports.iter().any(|import| import == flat) {
            return None;
        }
        if self.wildcard.iter().any(|wild| wild == package) {
            return None;
        }
        if self.local_names.iter().any(|name| name == simple) {
            return None;
        }
        // A different type with the same simple name is already imported: an
        // automatic import would create an ambiguity.
        if self
            .imports
            .iter()
            .any(|import| import.rsplit('.').next() == Some(simple))
        {
            return None;
        }
        let mut new_text = String::new();
        if self.needs_leading_newline {
            new_text.push('\n');
        }
        new_text.push_str("import ");
        new_text.push_str(flat);
        new_text.push_str(";\n");
        Some(TextEdit {
            range: Range::point(self.insertion),
            new_text,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_after_the_import_block() {
        let text = "package p;\nimport java.util.List;\nclass T {}\n";
        let context = ImportContext::new(text, Vec::new(), true);
        let edit = context
            .edit_for("java.util.ArrayList", true)
            .expect("import edit");
        assert_eq!(edit.new_text, "import java.util.ArrayList;\n");
        assert_eq!(edit.range.start, Position::new(2, 0));
    }

    #[test]
    fn skips_visible_types() {
        let text = "import java.util.*;\nclass T {}\n";
        let context = ImportContext::new(text, Vec::new(), true);
        assert!(context.edit_for("java.util.ArrayList", true).is_none());
        let text = "import java.util.ArrayList;\nclass T {}\n";
        let context = ImportContext::new(text, Vec::new(), true);
        assert!(context.edit_for("java.util.ArrayList", true).is_none());
        let text = "package p;\nclass T {}\n";
        let context = ImportContext::new(text, Vec::new(), true);
        assert!(context.edit_for("java.lang.String", true).is_none());
        assert!(context.edit_for("p.ArrayList", true).is_none());
    }

    #[test]
    fn skips_conflicts_and_local_types() {
        let text = "import other.ArrayList;\nclass T {}\n";
        let context = ImportContext::new(text, Vec::new(), true);
        assert!(context.edit_for("java.util.ArrayList", true).is_none());
        let text = "class T {}\n";
        let context = ImportContext::new(text, vec!["ArrayList".to_string()], true);
        assert!(context.edit_for("java.util.ArrayList", true).is_none());
    }

    #[test]
    fn disabled_in_import_lines() {
        let text = "class T {}\n";
        let context = ImportContext::new(text, Vec::new(), false);
        assert!(context.edit_for("java.util.ArrayList", true).is_none());
    }
}
