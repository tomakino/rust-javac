//! Quick fixes and refactorings.

use std::path::{Path, PathBuf};

use super::super::positions::word_at;
use super::super::{canonical, AnalysisSession};

impl AnalysisSession {
    /// Quick fixes at a position.
    pub(crate) fn code_actions_impl(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Vec<javac_analysis_api::CodeAction> {
        let key = PathBuf::from(canonical(file));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())
            .unwrap_or_default();
        let Some(word) = word_at(
            &text,
            position.line as usize + 1,
            position.column as usize + 1,
        ) else {
            return Vec::new();
        };
        if word.is_empty() || !word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
            return Vec::new();
        }
        let already_imported = text.lines().any(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("import ") && trimmed.ends_with(&format!("{word};"))
        });
        if already_imported {
            return Vec::new();
        }
        for sym in self.compiler.syms.get_all_classes() {
            let simple = self
                .compiler
                .ctx
                .sym(sym)
                .name
                .to_java_string()
                .to_string_lossy();
            if simple != word {
                continue;
            }
            let flat = self.compiler.ctx.sym(sym).flat_name().to_string();
            if !flat.contains('.') {
                continue;
            }
            let insert_line = text
                .lines()
                .position(|line| line.trim_start().starts_with("package "))
                .map(|line| line as u32 + 1)
                .unwrap_or(0);
            let edit = javac_analysis_api::TextEdit {
                range: javac_analysis_api::Range::point(javac_analysis_api::Position::new(
                    insert_line,
                    0,
                )),
                new_text: format!("import {flat};\n"),
            };
            return vec![javac_analysis_api::CodeAction {
                title: format!("Import '{flat}'"),
                edits: vec![edit],
            }];
        }
        Vec::new()
    }

    /// Rename the symbol at a position.
    pub(crate) fn rename_impl(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
        new_name: &str,
    ) -> Vec<javac_analysis_api::TextEdit> {
        let key = PathBuf::from(canonical(file));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())
            .unwrap_or_default();
        let line = position.line as usize + 1;
        let column = position.column as usize + 1;
        let Some(word) = word_at(&text, line, column) else {
            return Vec::new();
        };
        let word_len: u32 = word.chars().map(|c| c.len_utf16() as u32).sum();
        let locations = self.references_impl(file, position);
        locations
            .into_iter()
            .map(|location| {
                let start = location.range.start;
                javac_analysis_api::TextEdit {
                    range: javac_analysis_api::Range::new(
                        start,
                        javac_analysis_api::Position::new(start.line, start.column + word_len),
                    ),
                    new_text: new_name.to_string(),
                }
            })
            .collect()
    }
}
