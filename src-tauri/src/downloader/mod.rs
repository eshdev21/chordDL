use crate::config::{get_default_download_path, get_default_video_path};
use crate::dependencies::get_ytdlp_path;
use crate::error::{AppError, AppResult};
use crate::state::{AppState, DownloadStatus, DownloadTask};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use url::Url;
use uuid::Uuid;

pub mod args;
pub mod cleanup;
pub mod cookies;
pub mod init;
pub mod ytdlp;

// Re-export commonly used functions if needed, or keep them in their modules
pub use cleanup::cleanup_orphans;
pub use cookies::check_firefox_auth;
pub use init::{get_active_downloads, initialize_app};

// --- Configuration Constants ---

/// Retrieve all downloads from the application state (including queued/paused).
#[tauri::command]
pub fn get_all_downloads(app: AppHandle) -> Vec<DownloadTask> {
    app.state::<AppState>().get_all_tasks()
}

#[derive(Debug, serde::Deserialize)]
pub struct DownloadRequest {
    pub url: String,
    pub media_type: String,
    pub format: String,
    pub quality: String,
    pub output_path: String,
    pub is_playlist: bool,
    pub existing_id: Option<String>,
}

/// Download a single track or playlist from YouTube
#[tauri::command]
pub async fn download_single(app: AppHandle, request: DownloadRequest) -> AppResult<String> {
    let state = app.state::<AppState>();

    // 1. Resolve ID
    let task_id = request
        .existing_id
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // 2. Empty URL Check
    if request.url.trim().is_empty() {
        return Err(AppError::Internal("URL cannot be empty".to_string()));
    }

    // 2b. Block pure playlist URLs in single mode
    if !request.is_playlist {
        if let Ok(parsed) = Url::parse(&request.url) {
            let has_list = parsed.query_pairs().any(|(k, _)| k == "list");
            let has_video = parsed.query_pairs().any(|(k, _)| k == "v");
            if has_list && !has_video {
                return Err(AppError::PlaylistUrlInSingleMode);
            }
        }
    }

    // 3. CHECK FOR EXISTING STATE (Race Condition Fix)
    // We must check if this task is already running to prevent "Zombie" processes.
    {
        let inner = state
            .inner
            .lock()
            .map_err(|e| AppError::Internal(format!("Lock error: {}", e)))?;
        if let Some(task) = inner.downloads.get(&task_id) {
            match task.status {
                // If already active, REJECT the request.
                DownloadStatus::Downloading { .. }
                | DownloadStatus::Merging { .. }
                | DownloadStatus::Finalizing { .. }
                | DownloadStatus::Starting
                | DownloadStatus::FetchingMetadata => {
                    return Err(AppError::Internal(format!(
                        "Task '{}' is already active. Please wait.",
                        task.title
                    )));
                }
                // If queued, we can arguably ignore or let it be.
                // Currently, we'll reject to avoid double-queueing the same ID into the semaphore.
                DownloadStatus::Queued => {
                    return Err(AppError::Internal("Task is already queued.".to_string()));
                }
                // If Interrupted/Paused/Failed/Completed/Cancelled, we allow restart.
                // We fall through to the rest of the logic.
                _ => {}
            }
        }
    }

    let ytdlp_path = get_ytdlp_path(&app);
    if !ytdlp_path.exists() {
        return Err(AppError::DependencyMissing(
            "yt-dlp not installed".to_string(),
        ));
    }

    // 4. Setup paths (Reuse check)
    // If it's a resume, we should ideally reuse the OLD path, but the frontend sends the config path.
    // For now, we trust the frontend's output_path or default.
    let output_dir = if request.output_path.is_empty() {
        if request.media_type == "video" {
            get_default_video_path()
        } else {
            get_default_download_path()
        }
    } else {
        request.output_path.clone()
    };

    let workspace = prepare_workspace(&app, &task_id)?;
    let workspace_str = workspace.to_string_lossy().to_string();

    // Guard: abort if task was cancelled before it even entered the queue
    if !is_task_runnable(&state, &task_id) {
        let _ = std::fs::remove_dir_all(&workspace);
        return Ok(task_id);
    }

    let auth_enabled = state
        .inner
        .lock()
        .map(|i| i.config.cookies_enabled)
        .unwrap_or(false);
    let args = args::build_ytdlp_args(
        &app,
        request.url.clone(),
        request.media_type.clone(),
        request.format.clone(),
        request.quality.clone(),
        request.is_playlist,
        workspace,
        auth_enabled,
    );

    // 6. Create/Update Task in State
    // We do this BEFORE the async spawn to ensure the UI sees "Queued" immediately.
    // IMPORTANT: All checks below are ATOMIC with the insert (same lock scope)
    // to prevent the TOCTOU race where two concurrent async calls both pass
    // the early checks above and both insert tasks.
    {
        let mut inner = state
            .inner
            .lock()
            .map_err(|e| AppError::Internal(format!("Lock error: {}", e)))?;

        // Atomic re-check: task may have been inserted by a concurrent call
        if let Some(existing) = inner.downloads.get(&task_id) {
            match existing.status {
                DownloadStatus::Downloading { .. }
                | DownloadStatus::Merging { .. }
                | DownloadStatus::Finalizing { .. }
                | DownloadStatus::Starting
                | DownloadStatus::FetchingMetadata
                | DownloadStatus::Queued => {
                    return Err(AppError::Internal(format!(
                        "Task '{}' is already active or queued.",
                        existing.title
                    )));
                }
                _ => {}
            }
        }

        // Atomic re-check: duplicate URL (inside same lock as insert)
        if request.existing_id.is_none() {
            let is_dup = inner.downloads.values().any(|t| {
                t.url == request.url
                    && !matches!(
                        t.status,
                        DownloadStatus::Completed { .. }
                            | DownloadStatus::Failed { .. }
                            | DownloadStatus::Cancelled
                    )
            });
            if is_dup {
                return Err(AppError::Internal(
                    "This URL is already in the queue.".to_string(),
                ));
            }
        }

        // Preserve title/timestamp/children if resuming
        let (title, timestamp, children) = if let Some(old) = inner.downloads.get(&task_id) {
            (old.title.clone(), old.timestamp, old.children.clone())
        } else {
            (
                "Fetching metadata...".to_string(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                Vec::new(),
            )
        };

        let task = DownloadTask {
            id: task_id.clone(),
            url: request.url.clone(),
            title, // Keep old title if resuming
            media_type: request.media_type.clone(),
            format: request.format.clone(),
            quality: request.quality.clone(),
            output_path: output_dir.clone(),
            is_playlist: request.is_playlist,
            timestamp,
            pid: None,
            current_file: None,
            status: DownloadStatus::Queued,
            args: args.clone(),
            temp_path: workspace_str.clone(),
            children,
        };

        inner.downloads.insert(task_id.clone(), task.clone());
        if let Some(db_arc) = inner.db.as_ref() {
            let db_arc_clone = Arc::clone(db_arc);
            let task_clone = task.clone();
            tokio::task::spawn_blocking(move || {
                let db = db_arc_clone.lock().unwrap_or_else(|e| e.into_inner());
                let _ = db.save_task(&task_clone);
            });
        }
    }

    // 7. Emit events via Bus
    let event_bus = &state.event_bus;

    // Signal Queued state
    event_bus.emit(crate::events::AppEvent::DownloadStateChanged {
        id: task_id.clone(),
        status: DownloadStatus::Queued,
    });

    // Also emit created for NEW tasks so they get added to UI if missing
    if request.existing_id.is_none() {
        event_bus.emit(crate::events::AppEvent::DownloadCreated {
            id: task_id.clone(),
            title: "Queued...".to_string(),
            media_type: request.media_type.clone(),
            is_playlist: request.is_playlist,
            output_path: output_dir.clone(),
        });
    }

    let app_clone = app.clone();
    let task_id_clone = task_id.clone();
    let workspace_str_clone = workspace_str.clone();

    tauri::async_runtime::spawn(async move {
        let state = app_clone.state::<AppState>();

        // SEMAPHORE ACQUISITION
        // This is where it waits.
        let _permit = match state.download_semaphore.acquire().await {
            Ok(p) => p,
            Err(_) => return, // Semaphore closed
        };

        // Re-check state after acquiring permit (user might have cancelled while waiting)
        if !is_task_runnable(&state, &task_id_clone) {
            return;
        }

        let event_bus = state.event_bus.clone();

        event_bus.emit(crate::events::AppEvent::DownloadStateChanged {
            id: task_id_clone.clone(),
            status: DownloadStatus::Starting,
        });

        let result =
            ytdlp::run_ytdlp(&task_id_clone, &ytdlp_path, args, &output_dir, &event_bus).await;

        match result {
            Ok(filename) => {
                let mut final_meta = crate::state::PlaylistMetadata {
                    current_index: 0,
                    total_items: 0,
                    item_title: filename.clone(),
                };

                // Try to carry over last known item count to prevent progress drop in UI
                if let Some(task) = state.get_task(&task_id_clone) {
                    if let Some(meta) = task.status.playlist_meta() {
                        final_meta.current_index = meta.current_index;
                        final_meta.total_items = meta.total_items;
                    }
                }

                event_bus.emit(crate::events::AppEvent::DownloadStateChanged {
                    id: task_id_clone.clone(),
                    status: DownloadStatus::Finalizing {
                        playlist: Some(final_meta),
                    },
                });

                let workspace_owned = std::path::PathBuf::from(&workspace_str_clone);
                let destination_owned = std::path::PathBuf::from(&output_dir);

                let workspace_for_cleanup = workspace_owned.clone();
                let copy_result = tokio::task::spawn_blocking(move || {
                    copy_dir_all(&workspace_owned, &destination_owned)
                        .and_then(|_| std::fs::remove_dir_all(&workspace_for_cleanup))
                })
                .await;

                match copy_result {
                    Ok(Ok(_)) => {
                        event_bus.emit(crate::events::AppEvent::DownloadStateChanged {
                            id: task_id_clone.clone(),
                            status: DownloadStatus::Completed { filename },
                        });
                    }
                    _ => {
                        event_bus.emit(crate::events::AppEvent::DownloadStateChanged {
                            id: task_id_clone.clone(),
                            status: DownloadStatus::Failed {
                                reason: "Finalizing error: File copy failed".into(),
                                progress: 100.0,
                                category: crate::error::ErrorCategory::Generic,
                            },
                        });
                    }
                }
            }
            Err(e) => {
                handle_download_error(&state, &task_id_clone, e);
            }
        }
    });

    Ok(task_id)
}

/// Resume a paused or failed download by reconstructing the download request
/// and re-submitting it to the download pool.
#[tauri::command]
pub async fn resume_download(app: AppHandle, id: String) -> AppResult<()> {
    let state = app.state::<AppState>();
    // Get task from state - backend owns the data
    let task = {
        let inner = state
            .inner
            .lock()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        inner
            .downloads
            .get(&id)
            .cloned()
            .ok_or_else(|| AppError::Internal("Task not found".to_string()))?
    };

    // Use stored task data - no reconstruction needed
    download_single(
        app,
        DownloadRequest {
            url: task.url,
            media_type: task.media_type,
            format: task.format,
            quality: task.quality,
            output_path: task.output_path,
            is_playlist: task.is_playlist,
            existing_id: Some(id),
        },
    )
    .await?;

    Ok(())
}

/// Prepare an isolated workspace directory in the app cache for a specific task.
/// Returns the path to the task-specific UUID folder.
fn prepare_workspace(app: &AppHandle, task_id: &str) -> AppResult<PathBuf> {
    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let workspace = cache_dir.join("workspaces").join(task_id);

    std::fs::create_dir_all(&workspace).map_err(AppError::Io)?;
    Ok(workspace)
}

fn is_task_runnable(state: &AppState, task_id: &str) -> bool {
    state
        .get_task(task_id)
        .map(|t| matches!(t.status, DownloadStatus::Queued))
        .unwrap_or(false)
}

fn handle_download_error(state: &AppState, task_id: &str, e: AppError) {
    let task = state.get_task(task_id);
    let last_progress = match &task {
        Some(t) => t.status.calculate_global_progress(),
        None => 0.0,
    };

    if let Some(t) = task {
        if !matches!(
            t.status,
            DownloadStatus::Paused { .. } | DownloadStatus::Cancelled
        ) {
            state
                .event_bus
                .emit(crate::events::AppEvent::DownloadStateChanged {
                    id: task_id.to_string(),
                    status: DownloadStatus::Failed {
                        reason: e.user_message(),
                        progress: last_progress,
                        category: e.category(),
                    },
                });
        }
    }
}

/// Cancel an active download
#[tauri::command]
pub fn cancel_download(app: AppHandle, id: String) -> AppResult<()> {
    let state = app.state::<AppState>();
    let event_bus = &state.event_bus;

    // Get PID and workspace path before removing
    let task_info = state.get_task(&id).map(|t| (t.pid, t.temp_path.clone()));

    // Emit Cancelled FIRST so handle_download_error sees correct status
    event_bus.emit(crate::events::AppEvent::DownloadStateChanged {
        id: id.clone(),
        status: DownloadStatus::Cancelled,
    });

    event_bus.emit(crate::events::AppEvent::Log {
        source: "app".to_string(),
        level: "INFO".to_string(),
        message: format!("Task {} cancelled.", id),
    });

    // Kill process AFTER status is set
    if let Some((pid, temp_path)) = task_info {
        if let Some(pid) = pid {
            kill_task_process(pid);
            // Clear PID from state immediately to prevent redundant kills
            state.set_task_runtime_info(&id, Some(0), None);
        }
        cleanup::cleanup_workspace(&state, &temp_path);
    }

    Ok(())
}

/// Pause an active download by killing the yt-dlp process
#[tauri::command]
pub fn pause_download(app: AppHandle, id: String) -> AppResult<()> {
    let state = app.state::<AppState>();
    let event_bus = &state.event_bus;

    // Get PID before pausing
    let task = state.get_task(&id);
    let pid = task.as_ref().and_then(|t| t.pid);

    // Emit Paused FIRST so handle_download_error sees correct status
    let progress = task
        .as_ref()
        .map(|t| t.status.calculate_global_progress())
        .unwrap_or(0.0);

    event_bus.emit(crate::events::AppEvent::DownloadStateChanged {
        id: id.clone(),
        status: DownloadStatus::Paused { progress },
    });

    // Kill process AFTER status is set
    if let Some(pid) = pid {
        kill_task_process(pid);
        // Clear PID from state immediately
        state.set_task_runtime_info(&id, Some(0), None);
    }

    event_bus.emit(crate::events::AppEvent::Log {
        source: "app".to_string(),
        level: "INFO".to_string(),
        message: format!("Task {} paused.", id),
    });

    Ok(())
}

/// Helper to kill a task process (and its children) on Windows or Unix
/// uses sysinfo to verify the process is indeed yt-dlp/ffmpeg before killing
fn kill_task_process(pid_u32: u32) {
    if pid_u32 == 0 {
        return;
    }

    use sysinfo::{Pid, System};

    let mut sys = System::new_all();
    sys.refresh_all();

    let pid = Pid::from(pid_u32 as usize);

    if let Some(process) = sys.process(pid) {
        let name = process.name().to_string_lossy().to_lowercase();
        // Safety Check: Only kill if it's our expected binaries
        if name.contains("yt-dlp") || name.contains("ffmpeg") || name.contains("python") {
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                let _ = std::process::Command::new("taskkill")
                    .args(["/F", "/T", "/PID", &pid_u32.to_string()])
                    .creation_flags(0x08000000)
                    .status();
            }
            #[cfg(not(windows))]
            {
                let _ = std::process::Command::new("kill")
                    .arg("-9")
                    .arg(pid_u32.to_string())
                    .status();
            }
        }
    }
}

/// Helper to copy all files from source directory to destination directory recursively.
/// Skips 'archive.txt' as it is internal metadata used only during the download process.
fn copy_dir_all(
    src: impl AsRef<std::path::Path>,
    dst: impl AsRef<std::path::Path>,
) -> std::io::Result<()> {
    std::fs::create_dir_all(&dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            // Skip the download archive file
            if entry.file_name() == "archive.txt" {
                continue;
            }
            std::fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}
