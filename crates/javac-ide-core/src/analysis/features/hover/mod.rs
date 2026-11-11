//! Hover: structured signature, documentation, inherited documentation and
//! navigable overrides rendered to Markdown.

use std::path::{Path, PathBuf};

use javac_analysis_api::Location;

use super::super::positions::{in_literal_or_comment, line_offset};
use super::super::{canonical, FileAnalysis};

/// One overridden method shown in the hover.
struct OverrideSection {
    display: String,
    location: Option<Location>,
}

/// Hover is built as sections first, then rendered. Keeping the sections
/// distinct avoids sentinel checks (the old code treated the Overrides
/// separator as "documentation exists" and skipped index docs).
struct HoverSections {
    declaring_type: Option<String>,
    signature: String,
    documentation: Option<String>,
    inherited_from: Option<String>,
    overrides: Vec<OverrideSection>,
    source: Option<String>,
}

impl FileAnalysis {
    /// Hover contents for a position.
    pub(crate) fn hover_impl(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
        index: Option<&crate::workspace::index::model::ProjectIndex>,
    ) -> Option<String> {
        let key = PathBuf::from(canonical(file));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())?;
        let line = position.line as usize + 1;
        let column = position.column as usize + 1;
        if in_literal_or_comment(&text, line_offset(&text, line, column)) {
            return None;
        }

        let mut compiler_symbol = None;
        let symbol = match self
            .new_class_call_at(&text, position)
            .filter(|call| !call.anonymous && preceded_by_new_keyword(&text, position))
        {
            // `new T(...)`: the cursor is on the type name, but the useful
            // signature is the selected constructor.
            Some(call) => {
                compiler_symbol = Some(call.constructor);
                self.symbol_info(call.constructor, -1)?
            }
            None => {
                compiler_symbol = self.symbol_id_at(file, position).map(|(symbol, _)| symbol);
                FileAnalysis::symbol_at(self, file, line, column)?
            }
        };

        let index_symbol = index.and_then(|index| {
            let file_id = index.file_id_for_path(file)?;
            crate::workspace::index::query::symbol_at(index, file_id, &text, position)
        });
        let index_javadoc = index_symbol
            .and_then(|symbol| index.and_then(|index| index.symbol(symbol)))
            .and_then(|record| record.javadoc.clone());

        let mut documentation = symbol.doc.clone();
        let mut inherited_from = None;
        let mut hover_header = None;
        if let Some(symbol_id) = compiler_symbol {
            hover_header = self.hover_declaration(symbol_id, file);
            if let Some(resolved) = self.documentation_for_with_index(
                symbol_id,
                index,
                index_symbol,
                index_javadoc.as_deref(),
            ) {
                documentation = Some(resolved.text);
                inherited_from = resolved.inherited_from;
            }
        }
        if documentation.is_none() {
            documentation = index_javadoc;
        }

        let declaring_type = hover_header
            .as_ref()
            .and_then(|header| header.declaring_type.clone());
        let source = hover_header.as_ref().and_then(|header| header.source.clone());
        let mut signature = symbol.detail.clone();
        if let Some(header) = &hover_header {
            if symbol.kind == "class" {
                let keyword = class_keyword(self, compiler_symbol);
                signature = if header.modifiers.is_empty() {
                    format!("{keyword} {}", symbol.name)
                } else {
                    format!("{} {keyword} {}", header.modifiers, symbol.name)
                };
            } else if !header.modifiers.is_empty() {
                signature = format!("{} {}", header.modifiers, signature);
            }
        }

        let mut overrides: Vec<OverrideSection> = Vec::new();
        if symbol.kind == "method" {
            if let Some(symbol) = compiler_symbol {
                for entry in self.overridden_method_entries(symbol) {
                    overrides.push(OverrideSection {
                        display: entry.display,
                        location: entry.location,
                    });
                }
            }
            if let (Some(index), Some(index_symbol)) = (index, index_symbol) {
                let idx_entries = crate::workspace::index::query::overriding_method_entries(
                    index,
                    index_symbol,
                );
                for entry in idx_entries {
                    if let Some(existing) = overrides.iter_mut().find(|existing| {
                        crate::analysis::features::navigation::same_override_display(
                            &existing.display,
                            &entry.display,
                        )
                    }) {
                        // The index carries the fully qualified owner even when
                        // the compiler's flat name lost the package.
                        existing.display = entry.display;
                        existing.location = entry.location;
                        continue;
                    }
                    overrides.push(OverrideSection {
                        display: entry.display,
                        location: entry.location,
                    });
                }
            }
        }

        Some(render_hover(HoverSections {
            declaring_type,
            signature,
            documentation,
            inherited_from,
            overrides,
            source,
        }))
    }
}

fn class_keyword(analysis: &FileAnalysis, symbol: Option<crate::ids::SymbolId>) -> &'static str {
    use crate::code::flags;
    let Some(symbol) = symbol else {
        return "class";
    };
    let flags_value = analysis.compiler.ctx.sym(symbol).flags_field;
    if flags_value & flags::ANNOTATION as i64 != 0 {
        "@interface"
    } else if flags_value & flags::ENUM as i64 != 0 {
        "enum"
    } else if flags_value & flags::INTERFACE as i64 != 0 {
        "interface"
    } else if flags_value & flags::RECORD != 0 {
        "record"
    } else {
        "class"
    }
}

fn render_hover(sections: HoverSections) -> String {
    let newline = char::from(10);
    let mut text = String::new();
    if let Some(declaring_type) = sections.declaring_type {
        text.push_str(&declaring_type);
        text.push_str("\n\n");
    }
    text.push_str("```java");
    text.push(newline);
    text.push_str(&sections.signature);
    text.push(newline);
    text.push_str("```");

    if let Some(documentation) = sections
        .documentation
        .filter(|documentation| !documentation.trim().is_empty())
    {
        text.push_str("\n\n---\n\n");
        text.push_str(documentation.trim());
        if let Some(source) = sections.inherited_from {
            text.push_str("\n\n*Inherited from ");
            text.push_str(&source);
            text.push('*');
        }
    }

    if !sections.overrides.is_empty() {
        text.push_str("\n\n---\n\n**Overrides:**");
        for entry in sections.overrides.iter().take(5) {
            text.push_str("\n");
            text.push_str(&override_markdown(entry));
        }
        if sections.overrides.len() > 5 {
            text.push_str(&format!(
                "\n- … (+{} more)",
                sections.overrides.len() - 5
            ));
        }
    }
    if let Some(source) = sections.source {
        text.push_str("\n\n*Source: ");
        text.push_str(&source);
        text.push('*');
    }
    text
}

fn override_markdown(entry: &OverrideSection) -> String {
    match &entry.location {
        Some(location) => format!("- [{}]({})", entry.display, location_uri(location)),
        None => format!(
            "- {} *(declaration unavailable: no source location)*",
            entry.display
        ),
    }
}

/// `file://` URI with a line anchor; VS Code and other clients jump to the
/// declaration when the link is activated. Path encoding is shared with the
/// document-link service so `#`, `%`, spaces and non-ASCII names are safe.
fn location_uri(location: &Location) -> String {
    let mut uri = crate::analysis::uri::file_uri(&location.file);
    let start = location.range.start.line + 1;
    let end = location.range.end.line + 1;
    if start == end {
        uri.push_str(&format!("#L{start}"));
    } else {
        uri.push_str(&format!("#L{start}-L{end}"));
    }
    uri
}

/// Whether the cursor word is introduced by the `new` keyword. Enum
/// constants also have a synthetic constructor call, but their position is
/// not a `new` expression.
fn preceded_by_new_keyword(text: &str, position: javac_analysis_api::Position) -> bool {
    let line = position.line as usize + 1;
    let column = position.column as usize + 1;
    let offset = line_offset(text, line, column).min(text.len());
    let before = text[..offset].trim_end();
    let Some(prefix) = before.strip_suffix("new") else {
        return false;
    };
    match prefix.chars().next_back() {
        Some(c) => !(c.is_alphanumeric() || c == '_' || c == '$'),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_override_is_plain_text_with_reason() {
        let line = override_markdown(&OverrideSection {
            display: "Base#method()".to_string(),
            location: None,
        });
        assert_eq!(
            line,
            "- Base#method() *(declaration unavailable: no source location)*"
        );
    }

    #[test]
    fn location_uri_uses_a_line_anchor() {
        let location = Location {
            file: PathBuf::from("C:/tmp/Base.java"),
            range: javac_analysis_api::Range::new(
                javac_analysis_api::Position::new(3, 4),
                javac_analysis_api::Position::new(3, 9),
            ),
        };
        assert_eq!(
            location_uri(&location),
            "file:///C:/tmp/Base.java#L4"
        );
    }

    #[test]
    fn location_uri_percent_encodes_special_path_and_keeps_anchor() {
        let location = Location {
            file: PathBuf::from("C:/中文 目录/a#b%.java"),
            range: javac_analysis_api::Range::new(
                javac_analysis_api::Position::new(3, 4),
                javac_analysis_api::Position::new(3, 9),
            ),
        };
        let uri = location_uri(&location);
        let parsed = url::Url::parse(&uri).expect("hover link is a standard URI");
        assert_eq!(parsed.scheme(), "file");
        assert_eq!(parsed.fragment(), Some("L4"));
        assert!(uri.contains("%E4%B8%AD%E6%96%87%20%E7%9B%AE%E5%BD%95/a%23b%25.java#L4"), "{uri}");
    }
}
