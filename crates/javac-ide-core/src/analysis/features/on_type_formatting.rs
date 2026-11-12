//! On-type formatting: reindent the current line after `}`, `;` or a newline.
//!
//! The indentation model scans text with comments and string literals masked
//! out ([`crate::analysis::inspections::mask_non_code`]); bracket depth is a
//! structural property of that mask, not a semantic guess.

use std::path::{Path, PathBuf};

use javac_analysis_api::{Position, Range, TextEdit};

use super::super::{canonical, FileAnalysis};

const INDENT: usize = 4;

impl FileAnalysis {
    pub(crate) fn on_type_formatting_impl(
        &mut self,
        file: &Path,
        position: Position,
        character: char,
    ) -> Vec<TextEdit> {
        let _ = self.analyze(file);
        let key = PathBuf::from(canonical(file));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())
            .unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        let line_index = position.line as usize;
        let Some(line) = lines.get(line_index).copied() else {
            return Vec::new();
        };
        let masked = crate::analysis::inspections::mask_non_code(&text);
        let masked_lines: Vec<&str> = masked.lines().collect();
        let masked_line = masked_lines.get(line_index).copied().unwrap_or("");
        let tokens = super::completion::token_stream::tokenize(&self.compiler, &text);
        let token_on_line = |kind: crate::parser::tokens::TokenKind, line: usize| {
            tokens.iter().any(|token| {
                token.pos >= 0
                    && token.kind == kind
                    && crate::analysis::positions::offset_to_line_col(&text, token.pos as usize).0
                        as usize
                        == line
            })
        };
        let triggered = match character {
            '}' => token_on_line(crate::parser::tokens::TokenKind::RBRACE, line_index),
            ';' => token_on_line(crate::parser::tokens::TokenKind::SEMI, line_index),
            '\n' => true,
            _ => false,
        };
        if !triggered {
            return Vec::new();
        }
        // A line whose code part is empty lives inside a string or comment.
        if masked_line.trim().is_empty() && !line.trim().is_empty() {
            return Vec::new();
        }
        let indent = match character {
            '}' => depth_before(&masked_lines, line_index).saturating_sub(1) * INDENT,
            ';' => previous_indent(&lines, line_index),
            '\n' => {
                let previous = previous_indent(&lines, line_index);
                if opens_block(&tokens, &text, line_index)
                    || opens_case(&self.compiler.ctx, &text, line_index)
                {
                    previous + INDENT
                } else {
                    previous
                }
            }
            _ => return Vec::new(),
        };
        let current = leading_whitespace(line);
        if current == indent {
            return Vec::new();
        }
        vec![TextEdit {
            range: Range::new(
                Position::new(position.line, 0),
                Position::new(position.line, current as u32),
            ),
            new_text: " ".repeat(indent),
        }]
    }
}

fn leading_whitespace(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

fn depth_before(lines: &[&str], line_index: usize) -> usize {
    let mut depth = 0usize;
    for line in lines.iter().take(line_index) {
        for character in line.chars() {
            match character {
                '{' => depth += 1,
                '}' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
    }
    depth
}

fn previous_indent(lines: &[&str], line_index: usize) -> usize {
    line_index
        .checked_sub(1)
        .and_then(|index| lines.get(index))
        .map(|line| leading_whitespace(line))
        .unwrap_or(0)
}

fn opens_block(
    tokens: &[crate::parser::tokens::Token],
    text: &str,
    line_index: usize,
) -> bool {
    let Some(previous) = line_index.checked_sub(1) else {
        return false;
    };
    tokens.iter().any(|token| {
        token.pos >= 0
            && token.kind == crate::parser::tokens::TokenKind::LBRACE
            && crate::analysis::positions::offset_to_line_col(text, token.pos as usize).0
                as usize
                == previous
    })
}

/// Plan 9.2: a `case`/`default` AST node opens an indented body.
fn opens_case(
    ctx: &crate::compiler_context::CompilerContext,
    text: &str,
    line_index: usize,
) -> bool {
    let Some(previous) = line_index.checked_sub(1) else {
        return false;
    };
    ctx.trees.iter().any(|tree| {
        matches!(tree.kind, crate::tree::jc_tree::TreeKind::Case { .. })
            && tree.pos >= 0
            && crate::analysis::positions::offset_to_line_col(text, tree.pos as usize).0
                as usize
                == previous
    })
}
