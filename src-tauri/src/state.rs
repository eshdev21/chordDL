//! Centralized Application State
//!
//! Single source of truth for all download tasks and config.
//! The frontend never owns state — it only renders what the backend emits.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tokio::sync::Semaphore;

use crate::config::AppConfig;
use crate::error::ErrorCategory;
use crate::logger::Logger;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DependencyInstallState {
    pub target: String,
    pub status: String,
    pub progress: f64,
}

// ─── Core Types ─────────────────────────────────────────────

/// Global application state container utilizing thread-safe primitives.
pub struct AppState {
    /// Limit concurrent downloads (persistent Arc, resized dynamically)
    pub download_semaphore: Arc<Semaphore>,
    /// Consolidated application state (single lock for consistency)
    pub inner: Mutex<InnerState>,
    pub logger: Logger,
    pub event_bus: crate::events::EventBus,
}

/// Protected internal state containing all non-atomic application data.
pub struct InnerState {
    /// Active download tasks (keyed by task ID)
    pub downloads: HashMap<String, DownloadTask>,
    pub dependency_states: HashMap<String, DependencyInstallState>,
    pub config: AppConfig,
    pub current_limit: usize,
    pub db: Option<Arc<Mutex<crate::database::Db>>>,
    pub cookie_check_in_progress: bool,
    pub installing: HashSet<String>,
    pub last_update_check: Option<std::time::Instant>,
    pub cached_ytdlp_latest: Option<String>,
    pub cached_deno_latest: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            download_semaphore: Arc::new(Semaphore::new(3)), // Default limit of 3
            inner: Mutex::new(InnerState {
                downloads: HashMap::new(),
                dependency_states: HashMap::new(),
                config: AppConfig::default(),
                current_limit: 3,
                db: None,
                cookie_check_in_progress: false,
                installing: HashSet::new(),
                last_update_check: None,
                cached_ytdlp_latest: None,
                cached_deno_latest: None,
            }),
            logger: Logger::new(),
            event_bus: crate::events::EventBus::with_default_capacity(),
        }
    }
}

/// Metadata for a specific item within a batch/playlist
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PlaylistMetadata {
    pub current_index: usize,
    pub total_items: usize,
    pub item_title: String,
}

/// A sub-task representing an individual item in a playlist
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SubTask {
    pub id: String,
    pub title: String,
    pub status: DownloadStatus,
}

/// A download task with all metadata needed for resume
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DownloadTask {
    pub id: String,
    pub url: String,
    pub title: String,
    pub media_type: String,
    pub format: String,
    pub quality: String,
    pub output_path: String,
    pub is_playlist: bool,
    pub timestamp: u64,
    /// Arguments used to start yt-dlp (persisted for resume)
    #[serde(default)]
    pub args: Vec<String>,
    /// Unique workspace for this task (persisted)
    #[serde(default)]
    pub temp_path: String,
    /// Process ID of the yt-dlp child (runtime only, not persisted)
    #[serde(skip)]
    pub pid: Option<u32>,
    /// Current file being downloaded (runtime only, not persisted)
    #[serde(skip)]
    pub current_file: Option<PathBuf>,
    /// Current status
    pub status: DownloadStatus,
    /// Individual items if this is a playlist
    #[serde(default)]
    pub children: Vec<SubTask>,
}

/// State machine for download lifecycle
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(tag = "type", content = "data")]
#[derive(Default)]
pub enum DownloadStatus {
    Queued,
    #[default]
    Starting,
    FetchingMetadata,
    Downloading {
        progress: f64,
        speed: String,
        eta: String,
        playlist: Option<PlaylistMetadata>,
    },
    Merging {
        playlist: Option<PlaylistMetadata>,
    },
    Finalizing {
        playlist: Option<PlaylistMetadata>,
    },
    Completed {
        filename: String,
    },
    Failed {
        reason: String,
        progress: f64,
        category: ErrorCategory,
    },
    Cancelled,
    Paused {
        progress: f64,
    },
    /// Marked on startup for tasks that were in-progress when app closed
    Interrupted {
        progress: f64,
    },
}

impl DownloadStatus {
    /// Returns true if this state is "heavy" enough to warrant a database write.
    pub fn is_persisted(&self) -> bool {
        !matches!(self, DownloadStatus::Downloading { .. })
    }

    /// Returns true if the task should be removed from memory (completed/cancelled).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            DownloadStatus::Completed { .. } | DownloadStatus::Cancelled
        )
    }

    /// Centralized transition logic: returns true if a new status is allowed to overwrite the current one.
    pub fn can_be_overwritten_by(&self, new: &DownloadStatus) -> bool {
        // 1. Terminals are final for the life of the process
        if matches!(
            self,
            DownloadStatus::Completed { .. } | DownloadStatus::Cancelled
        ) {
            return false;
        }

        // 2. Manual states (Paused) block volatile updates
        if matches!(self, DownloadStatus::Paused { .. })
            && matches!(
                new,
                DownloadStatus::Downloading { .. }
                    | DownloadStatus::Merging { .. }
                    | DownloadStatus::FetchingMetadata
            )
        {
            return false;
        }

        // 3. Prevent demotion (e.g., Merging -> Downloading)
        // With hierarchical state, we still allow a Parent Task to move from Merging (of one item)
        // back to Downloading (for the next item) if it carries playlist metadata.
        match (self, new) {
            (DownloadStatus::Merging { .. }, DownloadStatus::Downloading { .. }) => {
                return true;
            }
            (DownloadStatus::Merging { .. }, DownloadStatus::FetchingMetadata) => {
                return true;
            }
            _ => {}
        }

        !matches!(
            (self, new),
            (
                DownloadStatus::Finalizing { .. },
                DownloadStatus::Downloading { .. }
                    | DownloadStatus::Merging { .. }
                    | DownloadStatus::FetchingMetadata
            ) | (
                DownloadStatus::Merging { .. },
                DownloadStatus::Downloading { .. } | DownloadStatus::FetchingMetadata
            ) | (
                DownloadStatus::Downloading { .. },
                DownloadStatus::FetchingMetadata
            ) | (
                DownloadStatus::Interrupted { .. },
                DownloadStatus::Downloading { .. }
            )
        )
    }

    /// Returns the playlist metadata if available for this state.
    pub fn playlist_meta(&self) -> Option<&PlaylistMetadata> {
        match self {
            DownloadStatus::Downloading { playlist, .. }
            | DownloadStatus::Merging { playlist, .. }
            | DownloadStatus::Finalizing { playlist, .. } => playlist.as_ref(),
            _ => None,
        }
    }

    /// Calculate a flattened "global" progress (0-100) for UI display.
    pub fn calculate_global_progress(&self) -> f64 {
        match self {
            DownloadStatus::Downloading {
                progress, playlist, ..
            } => {
                if let Some(p) = playlist {
                    if p.total_items > 0 {
                        return ((p.current_index as f64 - 1.0) * 100.0 + progress)
                            / p.total_items as f64;
                    }
                }
                *progress
            }
            DownloadStatus::Merging { playlist, .. }
            | DownloadStatus::Finalizing { playlist, .. } => {
                if let Some(p) = playlist {
                    if p.total_items > 0 {
                        // Merging/Finalizing is treated as the "completion" of that item's segment
                        return (p.current_index as f64 * 100.0) / p.total_items as f64;
                    }
                }
                100.0
            }
            DownloadStatus::Completed { .. } => 100.0,
            DownloadStatus::Failed { progress, .. }
            | DownloadStatus::Paused { progress }
            | DownloadStatus::Interrupted { progress } => *progress,
            _ => 0.0,
        }
    }

    /// Extract the current item title if available.
    pub fn item_title(&self) -> Option<String> {
        self.playlist_meta().map(|p| p.item_title.clone())
    }
}

#[derive(Clone, Serialize)]
pub struct StateChangeEvent {
    pub id: String,
    pub status: DownloadStatus,
    pub title: String,
    // Flattened fields for older UI or simpler consumption
    pub progress: f64,
    pub item_title: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct TaskCreatedEvent {
    pub id: String,
    pub title: String,
    pub media_type: String,
    pub is_playlist: bool,
    pub output_path: String,
}

// ─── State Operations ───────────────────────────────────────

impl AppState {
    /// Load persisted downloads from the database on startup.
    /// Any task that was in an active state gets marked Interrupted.
    pub fn load_from_disk(&self, _app: &AppHandle) {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let db_arc = match inner.db.as_ref() {
            Some(d) => Arc::clone(d),
            None => return,
        };

        // We can't hold the state lock across the blocking DB call
        drop(inner);

        let tasks_result = tokio::task::block_in_place(|| {
            let db = db_arc.lock().unwrap_or_else(|e| e.into_inner());
            db.load_active_tasks()
        });

        if let Ok(tasks) = tasks_result {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            for mut task in tasks {
                task.status = match task.status {
                    DownloadStatus::Queued
                    | DownloadStatus::Starting
                    | DownloadStatus::FetchingMetadata => {
                        DownloadStatus::Interrupted { progress: 0.0 }
                    }
                    DownloadStatus::Downloading { progress, .. } => {
                        DownloadStatus::Interrupted { progress }
                    }
                    DownloadStatus::Merging { .. } | DownloadStatus::Finalizing { .. } => {
                        DownloadStatus::Interrupted { progress: 99.0 }
                    }
                    other => other,
                };
                inner.downloads.insert(task.id.clone(), task);
            }
        }
    }

    /// Update the concurrent download limit safely and immediately.
    pub fn update_download_limit(&self, limit: usize) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let old = inner.current_limit;

        if limit > old {
            self.download_semaphore.add_permits(limit - old);
        } else if limit < old {
            self.download_semaphore.forget_permits(old - limit);
        }

        inner.current_limit = limit;
    }

    /// Persist the current download state to the database.
    pub fn persist_downloads(&self, _app: &AppHandle) {
        let payload = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            match inner.db.as_ref() {
                Some(db) => {
                    let tasks: Vec<DownloadTask> = inner
                        .downloads
                        .values()
                        .filter(|t| t.status.is_persisted())
                        .cloned()
                        .collect();
                    Some((tasks, Arc::clone(db)))
                }
                None => None,
            }
        };

        if let Some((tasks_to_save, db_arc)) = payload {
            tokio::task::spawn_blocking(move || {
                let db = db_arc.lock().unwrap_or_else(|e| e.into_inner());
                for task in tasks_to_save {
                    let _ = db.save_task(&task);
                }
            });
        }
    }

    /// Persist a single task to the database.
    pub fn persist_task(&self, id: &str) {
        let payload = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            match (inner.downloads.get(id), inner.db.as_ref()) {
                (Some(task), Some(db)) if task.status.is_persisted() => {
                    Some((task.clone(), Arc::clone(db)))
                }
                _ => None,
            }
        };

        if let Some((task_clone, db_arc_clone)) = payload {
            tokio::task::spawn_blocking(move || {
                let db = db_arc_clone.lock().unwrap_or_else(|e| e.into_inner());
                let _ = db.save_task(&task_clone);
            });
        }
    }

    /// Update a task's status, optionally persist, and emit event to frontend.
    pub fn update_task_status(&self, app: &AppHandle, id: &str, status: DownloadStatus) {
        let title;
        let should_persist;
        let is_terminal;

        {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let task = match inner.downloads.get_mut(id) {
                Some(t) => t,
                None => return,
            };

            if !task.status.can_be_overwritten_by(&status) {
                return;
            }

            task.status = status.clone();
            title = task.title.clone();
            should_persist = status.is_persisted();
            is_terminal = status.is_terminal();

            // 1. Propagate stop/error state to child playlist items
            propagate_stop_to_children(task, &status);

            // 2. Handle Terminal Purge
            if is_terminal {
                if let Some(db_arc) = inner.db.as_ref() {
                    let db_arc_clone = Arc::clone(db_arc);
                    let id_clone = id.to_string();
                    tokio::task::spawn_blocking(move || {
                        let db = db_arc_clone.lock().unwrap_or_else(|e| e.into_inner());
                        let _ = db.delete_task(&id_clone);
                    });
                }
                inner.downloads.remove(id);
            } else if let Some(meta) = status.playlist_meta() {
                update_playlist_children(task, &status, meta);
            }
        }

        if should_persist && !is_terminal {
            self.persist_task(id);
        }

        let log_msg = if let Some(meta) = status.playlist_meta() {
            format!(
                "Task {} (Playlist item {}/{}) status updated to: {:?} (Title: {})",
                id, meta.current_index, meta.total_items, status, title
            )
        } else {
            format!(
                "Task {} status updated to: {:?} (Title: {})",
                id, status, title
            )
        };
        self.logger.log_app("INFO", &log_msg);

        let global_progress = status.calculate_global_progress();
        let display_item_title = status.item_title();

        let _ = app.emit(
            "download:state-changed",
            StateChangeEvent {
                id: id.to_string(),
                status: status.clone(),
                title: title.clone(),
                progress: global_progress,
                item_title: display_item_title,
            },
        );
    }

    /// Update just the title of a task (when yt-dlp resolves the real filename).
    pub fn update_task_title(&self, app: &AppHandle, id: &str, title: &str) {
        let status;
        {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(task) = inner.downloads.get_mut(id) {
                task.title = title.to_string();
                status = task.status.clone();
            } else {
                return;
            }
        }
        self.persist_task(id);
        self.logger.log_app(
            "INFO",
            &format!(
                "Task {} title updated to: {} (Status: {:?})",
                id, title, status
            ),
        );
        let global_progress = status.calculate_global_progress();
        let display_item_title = status.item_title();

        let _ = app.emit(
            "download:state-changed",
            StateChangeEvent {
                id: id.to_string(),
                status,
                title: title.to_string(),
                progress: global_progress,
                item_title: display_item_title,
            },
        );
    }

    /// Update the PID and current file path for a task (runtime only).
    pub fn set_task_runtime_info(&self, id: &str, pid: Option<u32>, file: Option<PathBuf>) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(task) = inner.downloads.get_mut(id) {
            if let Some(p) = pid {
                task.pid = Some(p);
            }
            if let Some(f) = file {
                task.current_file = Some(f);
            }
        }
    }

    pub fn remove_task(&self, id: &str) -> Option<DownloadTask> {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .downloads
            .remove(id)
    }

    pub fn get_task(&self, id: &str) -> Option<DownloadTask> {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .downloads
            .get(id)
            .cloned()
    }

    pub fn get_all_tasks(&self) -> Vec<DownloadTask> {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .downloads
            .values()
            .cloned()
            .collect()
    }

    pub fn emit_config_change(&self, app: &AppHandle) {
        let config = self
            .inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .config
            .clone();
        let _ = app.emit("app:config-changed", config);
    }
}

// ─── Helpers ────────────────────────────────────────────────

fn propagate_stop_to_children(task: &mut DownloadTask, status: &DownloadStatus) {
    let is_stopping = status.is_terminal()
        || matches!(
            status,
            DownloadStatus::Failed { .. }
                | DownloadStatus::Interrupted { .. }
                | DownloadStatus::Paused { .. }
        );

    if is_stopping {
        for child in &mut task.children {
            if !child.status.is_terminal() {
                child.status = status.clone();
            }
        }
    }
}

fn update_playlist_children(
    task: &mut DownloadTask,
    status: &DownloadStatus,
    meta: &PlaylistMetadata,
) {
    let id = &task.id;

    // Mark previous items as completed
    for i in 1..meta.current_index {
        let prev_id = format!("{}-item-{}", id, i);
        if let Some(child) = task.children.iter_mut().find(|c| c.id == prev_id) {
            if !child.status.is_terminal() {
                child.status = DownloadStatus::Completed {
                    filename: child.title.clone(),
                };
            }
        }
    }

    // Update or Add Current Child SubTask
    let child_id = format!("{}-item-{}", id, meta.current_index);
    let new_child = SubTask {
        id: child_id.clone(),
        title: meta.item_title.clone(),
        status: status.clone(),
    };

    if let Some(existing_idx) = task.children.iter().position(|c| c.id == child_id) {
        task.children[existing_idx] = new_child;
    } else {
        task.children.push(new_child);
    }
}
