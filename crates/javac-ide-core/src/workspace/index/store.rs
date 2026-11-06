//! Publication of immutable index snapshots.
//!
//! Writers build a complete [`ProjectIndex`] off to the side and publish it as
//! one `Arc` swap. Readers clone the `Arc` and always see a consistent
//! snapshot; a build in flight keeps the previous snapshot visible.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use super::model::{IndexStatus, ProjectIndex};

pub struct IndexStore {
    snapshot: RwLock<Arc<ProjectIndex>>,
    building: AtomicBool,
}

impl IndexStore {
    pub fn new(index: ProjectIndex) -> Self {
        Self {
            snapshot: RwLock::new(Arc::new(index)),
            building: AtomicBool::new(false),
        }
    }

    pub fn empty(status: IndexStatus) -> Self {
        let placeholder = ProjectIndex {
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
        };
        Self::new(placeholder)
    }

    /// The current snapshot. The caller must treat it as read-only.
    pub fn snapshot(&self) -> Arc<ProjectIndex> {
        Arc::clone(&self.snapshot.read().unwrap_or_else(|poison| poison.into_inner()))
    }

    /// Whether a rebuild is currently running in the background.
    pub fn is_building(&self) -> bool {
        self.building.load(Ordering::Acquire)
    }

    pub fn begin_build(&self) {
        self.building.store(true, Ordering::Release);
    }

    pub fn end_build(&self) {
        self.building.store(false, Ordering::Release);
    }

    /// Atomically replace the snapshot after dropping every reader.
    pub fn publish(&self, index: ProjectIndex) {
        let mut slot = self
            .snapshot
            .write()
            .unwrap_or_else(|poison| poison.into_inner());
        *slot = Arc::new(index);
    }
}

impl std::fmt::Debug for IndexStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IndexStore")
            .field("building", &self.is_building())
            .finish()
    }
}
