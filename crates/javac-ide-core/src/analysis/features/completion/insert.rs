//! Insert-text generation for completion items.
//!
//! The rules follow the matrix in docs/IDE_COMPLETION_INSERT.md (section 5.2):
//! non-void methods never append a semicolon (except after return); void
//! methods always append one; the cursor only moves into the parentheses for
//! statement-position calls with at least one argument.

use javac_analysis_api::InsertTextFormat;

use super::context::InsertionContext;

/// The text a completion item inserts plus how to interpret it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InsertPlan {
    pub text: String,
    pub format: InsertTextFormat,
}

/// Escape snippet metacharacters in an identifier.
pub(crate) fn escape_snippet(name: &str) -> String {
    name.replace('$', "\\$")
}

/// Build the insertion plan for a method candidate.
pub(crate) fn method_plan(
    name: &str,
    is_void: bool,
    argument_count: usize,
    context: InsertionContext,
) -> InsertPlan {
    let name = escape_snippet(name);
    let has_arguments = argument_count > 0;
    match context {
        InsertionContext::TypeContext
        | InsertionContext::ImportPackage
        | InsertionContext::Annotation
        | InsertionContext::Unknown => InsertPlan {
            text: name,
            format: InsertTextFormat::PlainText,
        },
        InsertionContext::ReturnExpression => InsertPlan {
            text: if has_arguments {
                format!("{name}($0);")
            } else {
                format!("{name}();")
            },
            format: InsertTextFormat::Snippet,
        },
        InsertionContext::StatementStart => InsertPlan {
            text: if is_void {
                if has_arguments {
                    format!("{name}($0);")
                } else {
                    format!("{name}();")
                }
            } else if has_arguments {
                format!("{name}($0)")
            } else {
                format!("{name}()")
            },
            format: InsertTextFormat::Snippet,
        },
        InsertionContext::Expression => {
            if is_void {
                InsertPlan {
                    text: format!("{name}();"),
                    format: InsertTextFormat::Snippet,
                }
            } else {
                InsertPlan {
                    text: if has_arguments {
                        format!("{name}($0)")
                    } else {
                        format!("{name}()")
                    },
                    format: InsertTextFormat::Snippet,
                }
            }
        }
        InsertionContext::ClassBody => InsertPlan {
            text: if has_arguments {
                format!("{name}($0)")
            } else {
                format!("{name}()")
            },
            format: InsertTextFormat::Snippet,
        },
    }
}
