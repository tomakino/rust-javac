//! Workspace state and lifecycle.
//!
//! The workspace owns the only [`IndexStore`], the overlay buffers, the LRU of
//! transient analyses, and the background build handle. Index queries live in
//! [`super::service`]; this file only manages state transitions.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::analysis::{AnalysisConfig, FileAnalysis};
use super::cache::AnalysisCache;
use super::index::build::IndexLimits;
use super::index::key::FileId;
use super::index::model::{IndexStatus, ProjectIndex};
use super::index::{self, dump, IndexStore};
use super::model::{hash_text, WorkspaceModel};

pub struct Workspace {
    pub(crate) config: AnalysisConfig,
    pub(crate) model: WorkspaceModel,
    pub(crate) index: IndexStore,
    pub(crate) overlays: HashMap<PathBuf, String>,
    pub(crate) analyses: AnalysisCache,
    limits: IndexLimits,
    batch_size: usize,
    build: Option<std::thread::JoinHandle<index::IndexBuildResult>>,
    dirty: bool,
    /// (mtime, len) of every indexed file, used to detect disk edits without
    /// rereading the content.
    stamps: HashMap<PathBuf, (std::time::SystemTime, u64)>,
    completion_show_constructors: bool,
    /// Stable analysis slots for files that are not (yet) in the index.
    temporary_ids: HashMap<PathBuf, FileId>,
}

impl Workspace {
    pub fn new(config: AnalysisConfig) -> Self {
        Self::with_limits(
            config,
            IndexLimits::defaults(),
            index::DEFAULT_BATCH_SIZE,
        )
    }

    pub fn with_limits(config: AnalysisConfig, limits: IndexLimits, batch_size: usize) -> Self {
        let model = WorkspaceModel::from_config(&config);
        let index = if model.source_roots().is_empty() {
            IndexStore::empty(IndexStatus::Ready)
        } else if let Some(loaded) = dump::load(&model) {
            IndexStore::new(loaded)
        } else {
            IndexStore::empty(IndexStatus::Building)
        };
        let mut workspace = Self {
            config,
            model,
            index,
            overlays: HashMap::new(),
            analyses: AnalysisCache::new(),
            limits,
            batch_size,
            build: None,
            dirty: false,
            stamps: HashMap::new(),
            completion_show_constructors: false,
            temporary_ids: HashMap::new(),
        };
        workspace.update_stamps();
        workspace.start_background_build();
        workspace
    }

    pub fn config(&self) -> &AnalysisConfig {
        &self.config
    }

    pub fn model(&self) -> &WorkspaceModel {
        &self.model
    }

    /// The published index snapshot. Read-only; safe to keep while the next
    /// generation is built.
    pub fn index_snapshot(&self) -> Arc<ProjectIndex> {
        self.index.snapshot()
    }

    /// Toggle IDEA-style constructor expansion for `new` completion.
    pub fn set_completion_show_constructors(&mut self, enabled: bool) {
        self.completion_show_constructors = enabled;
        self.analyses.entries.clear();
    }

    pub fn is_building(&self) -> bool {
        self.index.is_building()
    }

    /// Record an open buffer so every query sees it instead of the disk text.
    pub fn set_overlay(&mut self, file: &Path, text: &str) {
        let path = PathBuf::from(crate::analysis::canonical(file));
        self.overlays.insert(path, text.to_string());
        self.dirty = true;
    }

    pub fn remove_overlay(&mut self, file: &Path) {
        let path = PathBuf::from(crate::analysis::canonical(file));
        if self.overlays.remove(&path).is_some() {
            self.dirty = true;
        }
    }

    /// Text the workspace currently associates with a file.
    pub fn text_of(&self, file: &Path) -> Option<String> {
        let path = PathBuf::from(crate::analysis::canonical(file));
        self.overlays
            .get(&path)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())
    }

    pub(crate) fn text_of_id(&self, file: FileId) -> Option<String> {
        let snapshot = self.index.snapshot();
        let record = snapshot.file(file)?;
        let path = record.path.clone();
        self.overlays
            .get(&path)
            .cloned()
            .or_else(|| std::fs::read_to_string(path).ok())
    }

    /// Start (or join) the background index build.
    fn start_background_build(&mut self) {
        if self.build.is_some() {
            return;
        }
        if self.model.source_roots().is_empty() {
            return;
        }
        self.index.begin_build();
        let config = self.config.clone();
        let model = self.model.clone();
        let overlays = self.overlays.clone();
        let limits = self.limits.clone();
        let batch_size = self.batch_size;
        self.build = Some(std::thread::spawn(move || {
            index::build::build(&model, &config, &overlays, batch_size, &limits)
        }));
    }

    /// Collect a finished build, publish it, and persist it.
    fn poll_build(&mut self) {
        let finished = self
            .build
            .as_ref()
            .map(|handle| handle.is_finished())
            .unwrap_or(false);
        if !finished {
            return;
        }
        let Some(handle) = self.build.take() else {
            return;
        };
        match handle.join() {
            Ok(result) => {
                self.publish_result(result);
            }
            Err(_) => {
                let snapshot = self.index.snapshot();
                // Keep the previous snapshot; only surface the failure when we
                // have no usable snapshot to fall back to.
                if snapshot.generation == 0 {
                    drop(snapshot);
                    self.index.publish(empty_index(IndexStatus::Failed(
                        "index build thread panicked".to_string(),
                    )));
                }
            }
        }
        self.index.end_build();
    }

    /// Make sure the index snapshot reflects the current sources.
    ///
    /// Disk edits and overlay changes mark the workspace dirty; the next
    /// index-backed query waits for the rebuild so it never answers from a
    /// stale snapshot.
    pub(crate) fn ensure_index(&mut self) {
        self.poll_build();
        self.detect_changes();
        if !self.dirty && self.index.snapshot().generation > 0 {
            return;
        }
        if self.build.is_none() {
            self.start_background_build();
        }
        self.join_build();
    }

    fn detect_changes(&mut self) {
        if self.index.snapshot().generation == 0 {
            return;
        }
        if self.sources_changed() {
            self.dirty = true;
        }
    }

    fn sources_changed(&self) -> bool {
        let snapshot = self.index.snapshot();
        if snapshot.files.is_empty() {
            return false;
        }
        let current = index::collect_java_files(&self.model.source_roots());
        if current.len() != snapshot.files.len() {
            return true;
        }
        for (path, record) in current.iter().zip(snapshot.files.iter()) {
            if path != &record.path {
                return true;
            }
            if let Some(text) = self.overlays.get(path) {
                if hash_text(text) != record.hash {
                    return true;
                }
                continue;
            }
            let Some((mtime, len)) = self.stamps.get(path) else {
                return true;
            };
            let Ok(meta) = std::fs::metadata(path) else {
                return true;
            };
            let changed = meta.modified().ok().as_ref() != Some(mtime) || meta.len() != *len;
            if changed {
                let hash = std::fs::read_to_string(path)
                    .map(|text| hash_text(&text))
                    .unwrap_or(0);
                if hash != record.hash {
                    return true;
                }
            }
        }
        for path in self.overlays.keys() {
            if snapshot.file_id_for_path(path).is_none() {
                return true;
            }
        }
        false
    }

    fn update_stamps(&mut self) {
        self.stamps.clear();
        let snapshot = self.index.snapshot();
        for record in &snapshot.files {
            let Ok(meta) = std::fs::metadata(&record.path) else {
                continue;
            };
            let Ok(mtime) = meta.modified() else {
                continue;
            };
            self.stamps.insert(record.path.clone(), (mtime, meta.len()));
        }
    }

    fn join_build(&mut self) {
        let Some(handle) = self.build.take() else {
            return;
        };
        if let Ok(result) = handle.join() {
            self.publish_result(result);
        }
        self.index.end_build();
    }

    fn publish_result(&mut self, result: index::IndexBuildResult) {
        let mut index = result.index;
        index.generation = self.index.snapshot().generation.saturating_add(1);
        if let Some(path) = dump::cache_path(dump::workspace_hash(&self.model)) {
            let _ = dump::save(&index, &path);
        }
        self.index.publish(index);
        self.dirty = false;
        self.update_stamps();
    }

    /// Force a rebuild after external changes (pom/classpath/file watchers).
    pub fn rebuild(&mut self) {
        self.poll_build();
        self.dirty = true;
        self.start_background_build();
    }

    /// Abandon a build that was started with a previous configuration.
    ///
    /// The worker thread keeps running (it cannot be cancelled), but its result
    /// is dropped when the join handle is replaced.
    pub(crate) fn discard_build(&mut self) {
        self.build = None;
        self.index.end_build();
    }

    /// Borrow (or create) the transient analysis for one file.
    ///
    /// A file keeps its slot across edits: the analysis is re-run against the
    /// new text, but per-file memory (inlay hints) survives.
    pub(crate) fn analysis(
        &mut self,
        file: &Path,
        text: Option<&str>,
    ) -> Option<&mut FileAnalysis> {
        let text = match text {
            Some(text) => text.to_string(),
            None => self.text_of(file)?,
        };
        let hash = hash_text(&text);
        let file_id = self.analysis_key(file);
        let key = PathBuf::from(crate::analysis::canonical(file));
        if let Some(mut analysis) = self.analyses.take(file_id) {
            if analysis.text_hash != hash {
                analysis.texts.insert(key.clone(), text.clone());
                analysis.compiler.overlay_text = Some((key, text));
                analysis.cache = None;
                analysis.attach(file_id, hash);
                let _ = analysis.analyze(file);
            }
            self.analyses.insert(file_id, analysis);
            return self.analysis_mut(file_id);
        }
        let mut analysis = FileAnalysis::new(self.config.clone());
        analysis.set_completion_show_constructors(self.completion_show_constructors);
        analysis.texts.insert(key.clone(), text.clone());
        analysis.compiler.overlay_text = Some((key, text));
        analysis.attach(file_id, hash);
        let _ = analysis.analyze(file);
        self.analyses.insert(file_id, analysis);
        self.analysis_mut(file_id)
    }

    /// Index id for an indexed file, or a stable synthetic id otherwise.
    fn analysis_key(&mut self, file: &Path) -> FileId {
        if let Some(id) = self.file_id(file) {
            return id;
        }
        let path = PathBuf::from(crate::analysis::canonical(file));
        if let Some(id) = self.temporary_ids.get(&path) {
            return *id;
        }
        let id = 0x8000_0000u32.wrapping_add(self.temporary_ids.len() as u32);
        self.temporary_ids.insert(path, id);
        id
    }

    fn analysis_mut(&mut self, file: FileId) -> Option<&mut FileAnalysis> {
        self.analyses
            .entries
            .iter_mut()
            .find(|(id, _)| *id == file)
            .map(|(_, analysis)| analysis)
    }

    fn file_id(&self, file: &Path) -> Option<FileId> {
        let snapshot = self.index.snapshot();
        snapshot.file_id_for_path(file)
    }

    pub(crate) fn with_analysis<T>(
        &mut self,
        file: &Path,
        text: Option<&str>,
        run: impl FnOnce(&mut FileAnalysis, Option<&ProjectIndex>) -> T,
    ) -> Option<T> {
        self.ensure_index();
        let snapshot = self.index.snapshot();
        let index = (!snapshot.files.is_empty()).then_some(snapshot.as_ref());
        let analysis = self.analysis(file, text)?;
        Some(run(analysis, index))
    }
}

pub(crate) fn empty_index(status: IndexStatus) -> ProjectIndex {
    ProjectIndex {
        generation: 0,
        workspace: Default::default(),
        strings: Default::default(),
        symbols: Vec::new(),
        by_key: Default::default(),
        by_name: Default::default(),
        by_file: Default::default(),
        by_supertype: Default::default(),
        occurrences: Default::default(),
        files: Vec::new(),
        status,
    }
}
