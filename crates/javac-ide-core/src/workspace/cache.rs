//! Bounded LRU of live per-file analyses.
//!
//! The workspace may keep at most a handful of [`FileAnalysis`] values alive;
//! they are transient and never stored in the index. A file keeps the same
//! slot across edits so caches such as the inlay-hint memory survive a
//! temporarily unparsable buffer.

use super::analysis::FileAnalysis;
use crate::workspace::index::key::FileId;

/// Maximum number of live per-file analyses.
pub(crate) const ANALYSIS_CACHE_CAPACITY: usize = 4;

pub(crate) struct AnalysisCache {
    pub(crate) entries: Vec<(FileId, FileAnalysis)>,
}

impl AnalysisCache {
    pub(crate) fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub(crate) fn take(&mut self, file: FileId) -> Option<FileAnalysis> {
        let position = self
            .entries
            .iter()
            .position(|(id, _)| *id == file)?;
        let (_, analysis) = self.entries.remove(position);
        Some(analysis)
    }

    pub(crate) fn insert(&mut self, file: FileId, analysis: FileAnalysis) {
        self.entries.retain(|(id, _)| *id != file);
        self.entries.push((file, analysis));
        while self.entries.len() > ANALYSIS_CACHE_CAPACITY {
            self.entries.remove(0);
        }
    }
}
