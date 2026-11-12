//! Auto-import edits for completion.
//!
//! IDEA adds the import statement when a completion inserts a type that is not
//! visible yet. The edit is attached to the completion item as an LSP
//! additionalTextEdits entry; the client applies both edits atomically.

use javac_analysis_api::{Position, Range, TextEdit};

use crate::analysis::features::import_syntax::ImportSyntax;

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
    /// Build from `PackageDecl` / `Import` AST nodes.
    ///
    /// `syntax.end` is the UTF-16 end of the last package/import declaration;
    /// the insertion point is that AST boundary, not a line scan.
    pub(crate) fn from_syntax(
        text: &str,
        syntax: &ImportSyntax,
        local_names: Vec<String>,
        enabled: bool,
    ) -> Self {
        let insert_utf16 = syntax.end;
        let (line, _) = crate::analysis::positions::offset_to_line_col(text, insert_utf16);
        let insertion = if insert_utf16 == 0 {
            Position::new(0, 0)
        } else {
            Position::new(line + 1, 0)
        };
        Self {
            package: syntax.package.clone(),
            imports: syntax.imports.clone(),
            wildcard: syntax.wildcard.clone(),
            local_names,
            // The AST end is the semicolon; the edit is inserted on the next
            // line, matching the package/import block boundary.
            insertion,
            needs_leading_newline: false,
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

    fn make_context(
        text: &str,
        package: &str,
        imports: &[&str],
        wildcard: &[&str],
        end: usize,
        enabled: bool,
    ) -> ImportContext {
        let syntax = ImportSyntax {
            package: package.to_string(),
            imports: imports.iter().map(|value| value.to_string()).collect(),
            wildcard: wildcard.iter().map(|value| value.to_string()).collect(),
            static_imports: Vec::new(),
            end,
        };
        ImportContext::from_syntax(text, &syntax, Vec::new(), enabled)
    }

    #[test]
    fn inserts_after_the_import_block() {
        let text = "package p;\nimport java.util.List;\nclass T {}\n";
        let context = make_context(
            text,
            "p",
            &["java.util.List"],
            &[],
            "package p;\nimport java.util.List;".len(),
            true,
        );
        let edit = context
            .edit_for("java.util.ArrayList", true)
            .expect("import edit");
        assert_eq!(edit.new_text, "import java.util.ArrayList;\n");
        assert_eq!(edit.range.start, Position::new(2, 0));
    }

    #[test]
    fn skips_visible_types() {
        let text = "import java.util.*;\nclass T {}\n";
        let context = make_context(text, "", &[], &["java.util"], 0, true);
        assert!(context.edit_for("java.util.ArrayList", true).is_none());
        let text = "import java.util.ArrayList;\nclass T {}\n";
        let context = make_context(text, "", &["java.util.ArrayList"], &[], 0, true);
        assert!(context.edit_for("java.util.ArrayList", true).is_none());
        let text = "package p;\nclass T {}\n";
        let context = make_context(text, "p", &[], &[], 0, true);
        assert!(context.edit_for("java.lang.String", true).is_none());
        assert!(context.edit_for("p.ArrayList", true).is_none());
    }

    #[test]
    fn skips_conflicts_and_local_types() {
        let text = "import other.ArrayList;\nclass T {}\n";
        let context = make_context(text, "", &["other.ArrayList"], &[], 0, true);
        assert!(context.edit_for("java.util.ArrayList", true).is_none());
        let text = "class T {}\n";
        let context = ImportContext::from_syntax(
            text,
            &ImportSyntax::default(),
            vec!["ArrayList".to_string()],
            true,
        );
        assert!(context.edit_for("java.util.ArrayList", true).is_none());
    }

    #[test]
    fn disabled_in_import_lines() {
        let text = "class T {}\n";
        let context = make_context(text, "", &[], &[], 0, false);
        assert!(context.edit_for("java.util.ArrayList", true).is_none());
    }
}
