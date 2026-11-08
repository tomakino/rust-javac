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
use super::index::extract::ExtractedFile;
use super::index::key::FileId;
use super::index::model::{IndexStatus, ProjectIndex, SourceSetKind};
use super::index::update::UpdateTask;
use super::index::{self, dump, IndexStore};
use super::model::{hash_text, WorkspaceModel};
use crate::project::model::ProjectModel;

pub struct Workspace {
    pub(crate) config: AnalysisConfig,
    pub(crate) model: WorkspaceModel,
    pub(crate) index: IndexStore,
    pub(crate) overlays: HashMap<PathBuf, String>,
    pub(crate) analyses: AnalysisCache,
    /// Index generation the live analyses were built against.
    analysis_generation: u64,
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
    incremental_updates: u64,
    full_rebuilds: u64,
    /// Test-only: delay the background build so a mid-build snapshot can be
    /// observed deterministically.
    build_delay: Option<std::time::Duration>,
}

/// Observable index counters, used by tests and the LSP stats request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexStats {
    pub generation: u64,
    pub files: usize,
    pub symbols: usize,
    pub incremental_updates: u64,
    pub full_rebuilds: u64,
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
        Self::from_parts(config, model, limits, batch_size)
    }

    /// Build a workspace from a build-system-neutral project model.
    ///
    /// The analysis configuration is derived from the merged class path and
    /// source roots; `source_level_override` wins over the provider value.
    pub fn new_from_project(
        project: &ProjectModel,
        output_dir: PathBuf,
        source_level_override: Option<String>,
    ) -> Self {
        let mut model = WorkspaceModel::from_project(project);
        if let Some(level) = source_level_override {
            model.source_level = level;
        }
        let mut class_path = model.class_path();
        // Dependencies are resolved from build output: the analysis compiler
        // must not see project sources (a `module-info.java` on the source
        // path hides JDK modules). The edited file is passed explicitly and
        // its overlay text still wins.
        for module in &project.modules {
            for set in &module.source_sets {
                if let Some(output) = &set.output
                    && output.exists()
                    && !class_path.contains(output)
                {
                    class_path.push(output.clone());
                }
            }
        }
        class_path.sort();
        let config = AnalysisConfig {
            class_path,
            output_dir,
            source_level: model.source_level.clone(),
            source_path: Vec::new(),
        };
        Self::from_parts(
            config,
            model,
            IndexLimits::defaults(),
            index::DEFAULT_BATCH_SIZE,
        )
    }

    fn from_parts(
        config: AnalysisConfig,
        model: WorkspaceModel,
        limits: IndexLimits,
        batch_size: usize,
    ) -> Self {
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
            analysis_generation: 0,
            limits,
            batch_size,
            build: None,
            dirty: false,
            stamps: HashMap::new(),
            completion_show_constructors: false,
            temporary_ids: HashMap::new(),
            incremental_updates: 0,
            full_rebuilds: 0,
            build_delay: None,
        };
        workspace.update_stamps();
        workspace.start_background_build();
        workspace
    }

    /// Whether `file` is an extracted JDK/classpath source under the output
    /// directory (`jdk-src`, `external-src`).
    ///
    /// Those files are read-only reference material; their dependencies are
    /// not on the project source path, so diagnosing them only produces
    /// spurious errors.
    pub fn is_extracted_source(&self, file: &Path) -> bool {
        let key = PathBuf::from(crate::analysis::canonical(file));
        let output = PathBuf::from(crate::analysis::canonical(&self.config.output_dir));
        ["jdk-src", "external-src"]
            .iter()
            .any(|dir| key.starts_with(output.join(dir)))
    }
    /// True when an open overlay differs from the indexed text.
    ///
    /// Write operations such as rename must not use stale occurrence ranges.
    pub(crate) fn has_unsaved_edits(&self) -> bool {
        let snapshot = self.index.snapshot();
        self.overlays.iter().any(|(path, text)| {
            snapshot
                .files
                .iter()
                .find(|record| record.path == *path)
                .is_some_and(|record| hash_text(text) != record.hash)
        })
    }
    /// True when the index may lag behind the overlays/disk.
    pub(crate) fn is_dirty(&self) -> bool {
        self.dirty
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

    /// Publish a finished background build (indexStats and LSP requests use it).
    pub fn poll_index(&mut self) {
        self.poll_build();
    }

    /// Snapshot counters for diagnostics and tests.
    pub fn index_stats(&self) -> IndexStats {
        let snapshot = self.index.snapshot();
        IndexStats {
            generation: snapshot.generation,
            files: snapshot.files.len(),
            symbols: snapshot.symbols.len(),
            incremental_updates: self.incremental_updates,
            full_rebuilds: self.full_rebuilds,
        }
    }

    /// Record an open buffer so every query sees it instead of the disk text.
    pub fn set_overlay(&mut self, file: &Path, text: &str) {
        let path = PathBuf::from(crate::analysis::canonical(file));
        self.overlays.insert(path, text.to_string());
        // The edited file keeps its analysis (inlay-hint memory); analyses of
        // other files may have compiled against the previous content.
        self.retain_analysis_for(file);
        self.dirty = true;
    }

    pub fn remove_overlay(&mut self, file: &Path) {
        let path = PathBuf::from(crate::analysis::canonical(file));
        let removed = self.overlays.remove(&path).is_some();
        if removed {
            self.retain_analysis_for(file);
            self.dirty = true;
        }
    }

    /// Keep only the edited file's analysis.
    ///
    /// The edited file keeps its analysis (and diagnostics cache) so an
    /// unchanged text does not trigger a re-analysis; a changed text is
    /// recompiled by [`Self::analysis`] through its text-hash comparison.
    fn retain_analysis_for(&mut self, file: &Path) {
        let edited_id = self.analysis_key(file);
        self.analyses.entries.retain(|(id, _)| *id == edited_id);
    }

    /// Replace the analysis class/source path (for example when build
    /// artifacts become available). Transient analyses are dropped so the
    /// next query uses the new configuration; the index is unaffected.
    pub fn update_analysis_config(&mut self, class_path: Vec<PathBuf>, source_path: Vec<PathBuf>) {
        if self.config.class_path == class_path && self.config.source_path == source_path {
            return;
        }
        self.config.class_path = class_path;
        self.config.source_path = source_path;
        self.analyses.entries.clear();
    }

    /// Drop the transient analysis for one file (its class files changed).
    ///
    /// The closure files come from the published snapshot, so match their
    /// canonical paths directly and only fall back to `canonicalize` for
    /// unknown paths (it is expensive to call per file on Windows).
    pub fn invalidate_analysis(&mut self, file: &Path) {
        let id = {
            let snapshot = self.index.snapshot();
            snapshot
                .files
                .iter()
                .position(|record| record.path == file)
                .map(|index| index as FileId)
        };
        let id = match id {
            Some(id) => Some(id),
            None => self.file_id(file),
        };
        let Some(id) = id else {
            return;
        };
        self.analyses.entries.retain(|(entry, _)| *entry != id);
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
        let delay = self.build_delay;
        self.build = Some(std::thread::spawn(move || {
            if let Some(delay) = delay {
                std::thread::sleep(delay);
            }
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
    /// A content change in an already-indexed file is applied in place
    /// ([`try_incremental_update`]); anything else (new/deleted files, failed
    /// extraction, unsafe edits) falls back to a background full rebuild.
    pub(crate) fn ensure_index(&mut self) {
        self.poll_build();
        // A build in flight: keep serving the previous snapshot (or fall back
        // to FileAnalysis when there is none) instead of blocking the query.
        if self.build.is_some() {
            return;
        }
        if self.index.snapshot().generation > 0 {
            let changed = self.changed_files();
            if changed.is_empty() {
                self.dirty = false;
                return;
            }
            if self.try_incremental_update(&changed) {
                return;
            }
        }
        // Start the rebuild in the background; do not wait for it.
        self.start_background_build();
    }

    /// Block until the index reflects the current sources.
    ///
    /// Queries deliberately do not wait for a background build; callers that
    /// need the new snapshot (LSP initialization, tests) use this explicitly.
    pub fn wait_for_index(&mut self) {
        loop {
            self.poll_build();
            if self.build.is_some() {
                self.join_build();
                continue;
            }
            if self.index.snapshot().generation > 0 {
                let changed = self.changed_files();
                if changed.is_empty() {
                    self.dirty = false;
                    return;
                }
                if self.try_incremental_update(&changed) {
                    return;
                }
            }
            self.start_background_build();
            if self.build.is_none() {
                return;
            }
        }
    }

    /// Test-only hook: delay the background build so a mid-build snapshot can
    /// be observed deterministically. Production callers leave this unset.
    #[doc(hidden)]
    pub fn set_build_delay_for_tests(&mut self, delay: std::time::Duration) {
        self.build_delay = Some(delay);
    }

    /// Test-only hook: drop the published snapshot so the next query has to
    /// fall back to `FileAnalysis`.
    #[doc(hidden)]
    pub fn clear_index_for_tests(&mut self) {
        self.index.publish(empty_index(IndexStatus::Building));
        self.stamps.clear();
        self.dirty = true;
    }

    /// Try to apply incremental updates for `changed` files.
    ///
    /// Content changes are applied in place. When a file's declaration shape
    /// changes, its reverse-dependency closure is re-extracted first, so the
    /// batch stays consistent without a full project rebuild. Added files get
    /// a new `FileId`; deleted files are tombstoned and their references
    /// cleared. `false` means the batch is unsafe and the caller rebuilds.
    fn try_incremental_update(&mut self, changed: &[PathBuf]) -> bool {
        let snapshot = self.index.snapshot();
        let mut updates: Vec<(FileId, PathBuf)> = Vec::new();
        let mut adds: Vec<PathBuf> = Vec::new();
        let mut deletes: Vec<FileId> = Vec::new();
        for path in changed {
            match snapshot.file_id_for_path(path) {
                Some(file) => {
                    // A skipped record (overlay-only build) has no occurrences;
                    // only a full rebuild restores them.
                    if snapshot.file(file).is_some_and(|record| !record.status.is_live()) {
                        return false;
                    }
                    if self.text_of(path).is_some() {
                        updates.push((file, path.clone()));
                    } else {
                        deletes.push(file);
                    }
                }
                None => adds.push(path.clone()),
            }
        }
        if updates.is_empty() && adds.is_empty() && deletes.is_empty() {
            return false;
        }

        // Extract the changed files and decide which dependents join the batch.
        let mut extracted: HashMap<FileId, (u64, ExtractedFile)> = HashMap::new();
        let mut roots: Vec<FileId> = Vec::new();
        for (file, path) in &updates {
            let Some(text) = self.text_of(path) else {
                return false;
            };
            let extraction = index::extract::extract_file(&self.config, path, *file, &text);
            if extraction.failed {
                return false;
            }
            if index::update::declarations_changed(&snapshot, *file, &extraction) {
                roots.push(*file);
            }
            extracted.insert(*file, (hash_text(&text), extraction));
        }

        // Declaration-changing files go first; their dependents follow.
        let mut order: Vec<FileId> = roots.clone();
        for (file, _) in &updates {
            if !order.contains(file) {
                order.push(*file);
            }
        }
        if !roots.is_empty() {
            // Two changed files that reference each other cannot be ordered
            // safely; let the full rebuild handle it.
            for a in &roots {
                for b in &roots {
                    if a != b && snapshot.dependent_closure(*a).contains(b) {
                        return false;
                    }
                }
            }
            for root in &roots {
                for member in snapshot.dependent_closure(*root) {
                    if member == *root || order.contains(&member) || deletes.contains(&member) {
                        continue;
                    }
                    let Some(record) = snapshot.file(member) else {
                        return false;
                    };
                    if !record.status.is_live() {
                        continue;
                    }
                    let Some(text) = self.text_of(&record.path) else {
                        return false;
                    };
                    let extraction = index::extract::extract_file(
                        &self.config,
                        &record.path,
                        member,
                        &text,
                    );
                    if extraction.failed {
                        return false;
                    }
                    extracted.insert(member, (hash_text(&text), extraction));
                    order.push(member);
                }
            }
        }

        let mut tasks: Vec<UpdateTask> = Vec::new();
        for file in &order {
            let Some((hash, extraction)) = extracted.remove(file) else {
                return false;
            };
            tasks.push(UpdateTask::Update {
                file: *file,
                text_hash: hash,
                extraction,
            });
        }
        let mut next_file = snapshot.files.len() as FileId;
        for path in &adds {
            let Some(text) = self.text_of(path) else {
                return false;
            };
            let extraction = index::extract::extract_file(&self.config, path, next_file, &text);
            if extraction.failed {
                return false;
            }
            let source_set = self
                .model
                .source_set_for(path)
                .unwrap_or(SourceSetKind::Main);
            tasks.push(UpdateTask::Add {
                file: next_file,
                path: path.clone(),
                source_set,
                text_hash: hash_text(&text),
                extraction,
            });
            next_file += 1;
        }
        for file in deletes {
            tasks.push(UpdateTask::Delete { file });
        }

        let Some(mut next) = index::update::apply_batch(&snapshot, &tasks) else {
            return false;
        };
        next.generation = snapshot.generation.saturating_add(1);
        if let Some(cache) = index::dump::cache_path(index::dump::workspace_hash(&self.model)) {
            let _ = index::dump::save(&next, &cache);
        }
        self.index.publish(next);
        self.update_stamps();
        self.dirty = false;
        self.incremental_updates += 1;
        true
    }

    /// Files whose content no longer matches the published snapshot.
    ///
    /// Also reports added (`path` not in the snapshot) and removed files; the
    /// incremental updater rejects both and the caller rebuilds.
    fn changed_files(&self) -> Vec<PathBuf> {
        let snapshot = self.index.snapshot();
        if snapshot.files.is_empty() {
            return Vec::new();
        }
        let current = index::collect_java_files(&self.model.source_roots());
        let mut changed = Vec::new();
        let mut present: Vec<&PathBuf> = current.iter().collect();
        for record in &snapshot.files {
            if !record.status.is_live() {
                // A skipped file is still pending: while an overlay lasts it is
                // analysis-only, but once the overlay is gone the saved bytes
                // must be indexed.
                if self.overlays.contains_key(&record.path) {
                    continue;
                }
                if matches!(
                    record.status,
                    crate::workspace::index::model::FileStatus::Skipped
                ) && record.path.exists()
                {
                    changed.push(record.path.clone());
                }
                continue;
            }
            let Some(position) = present.iter().position(|path| **path == record.path) else {
                changed.push(record.path.clone()); // deleted
                continue;
            };
            present.remove(position);
            if let Some(text) = self.overlays.get(&record.path) {
                if hash_text(text) != record.hash {
                    changed.push(record.path.clone());
                }
                continue;
            }
            let Some((mtime, len)) = self.stamps.get(&record.path) else {
                changed.push(record.path.clone());
                continue;
            };
            let Ok(meta) = std::fs::metadata(&record.path) else {
                changed.push(record.path.clone());
                continue;
            };
            let touched =
                meta.modified().ok().as_ref() != Some(mtime) || meta.len() != *len;
            if touched {
                let hash = std::fs::read_to_string(&record.path)
                    .map(|text| hash_text(&text))
                    .unwrap_or(0);
                if hash != record.hash {
                    changed.push(record.path.clone());
                }
            }
        }
        // Any file left over was added after the snapshot.
        changed.extend(present.into_iter().cloned());
        changed
    }

    fn update_stamps(&mut self) {
        self.stamps.clear();
        let snapshot = self.index.snapshot();
        for record in &snapshot.files {
            if !record.status.is_live() {
                continue;
            }
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
        self.full_rebuilds += 1;
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
        let provided = text.is_some();
        let text = match text {
            Some(text) => text.to_string(),
            None => self.text_of(file)?,
        };
        let overlays = self.overlays.clone();
        let hash = hash_text(&text);
        let file_id = self.analysis_key(file);
        let key = PathBuf::from(crate::analysis::canonical(file));
        if let Some(mut analysis) = self.analyses.take(file_id) {
            if analysis.text_hash != hash {
                analysis.texts = overlays.clone();
                analysis.texts.insert(key.clone(), text.clone());
                analysis.compiler.overlay_text = if provided || overlays.contains_key(&key) {
                    Some((key.clone(), text.clone()))
                } else {
                    None
                };
                analysis.compiler.set_overlays(&overlays);
                analysis.cache = None;
                analysis.attach(file_id, hash);
                // Re-analyzing a changed text reuses the old compiler round,
                // which reported spurious override errors for class-path
                // dependencies; a fresh compiler round matches javac.
                if analysis.initialized {
                    // A fresh compiler round matches javac; reusing the round
                    // reported spurious override errors for class-path
                    // dependencies. Hint memory survives the rebuild.
                    let hints = std::mem::take(&mut analysis.inlay_hint_cache);
                    analysis.rebuild();
                    analysis.inlay_hint_cache = hints;
                    analysis.texts = overlays.clone();
                    analysis.texts.insert(key.clone(), text.clone());
                    analysis.compiler.overlay_text = Some((key.clone(), text.clone()));
                    analysis.compiler.set_overlays(&overlays);
                }
                let _ = analysis.analyze(file);
            } else {
                analysis.compiler.set_overlays(&overlays);
                // Only unsaved text needs the overlay parse path; a file read
                // from disk must go through the file manager so path identity
                // (binary name / module queries) stays intact.
                analysis.compiler.overlay_text = if provided || overlays.contains_key(&key) {
                    Some((key.clone(), text.clone()))
                } else {
                    None
                };
            }
            self.analyses.insert(file_id, analysis);
            return self.analysis_mut(file_id);
        }
        let mut analysis = FileAnalysis::new(self.config.clone());
        analysis.set_completion_show_constructors(self.completion_show_constructors);
        analysis.texts = overlays.clone();
        analysis.texts.insert(key.clone(), text.clone());
        analysis.compiler.overlay_text = if provided || overlays.contains_key(&key) {
            Some((key, text))
        } else {
            None
        };
        analysis.compiler.set_overlays(&overlays);
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
        // A new index generation means other files changed: rebuild the
        // transient analyses so diagnostics/completion see the new state.
        if snapshot.generation != self.analysis_generation {
            self.analyses.entries.clear();
            self.analysis_generation = snapshot.generation;
        }
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
        dependents: Default::default(),
        files: Vec::new(),
        status,
    }
}
