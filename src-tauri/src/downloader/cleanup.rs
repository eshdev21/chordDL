use crate::state::AppState;
use std::fs;
use tauri::{AppHandle, Manager};

/// Cleanup the entire workspace for a task
pub fn cleanup_workspace(state: &AppState, temp_path: &str) {
    let path = std::path::Path::new(temp_path);
    if path.exists() && path.is_dir() {
        state
            .logger
            .log_app("INFO", &format!("Cleaning up workspace: {}", temp_path));
        let _ = fs::remove_dir_all(path);
    }
}

/// Cleanup orphan workspaces and stale JSON entries
#[tauri::command]
pub async fn cleanup_orphans(app: AppHandle) -> Result<String, String> {
    let state = app.state::<AppState>();
    state
        .logger
        .log_app("INFO", "Starting deterministic workspace-based cleanup...");

    // 1. Get IDs of tasks that should still have workspaces (i.e. not terminal)
    // We only want to PROTECT folders for tasks that are runnable (Queued, Failed, etc.)
    let all_tasks = state.get_all_tasks();
    let protected_ids: Vec<String> = all_tasks
        .iter()
        .filter(|t| {
            // Protect everything EXCEPT terminal states (Completed/Cancelled).
            // Interrupted, Paused, Failed tasks need their workspaces for resume.
            !t.status.is_terminal()
        })
        .map(|t| t.id.clone())
        .collect();

    // 2. Identify central workspace root in app cache
    let workspace_root = match app.path().app_cache_dir() {
        Ok(d) => d,
        Err(_) => return Err("Could not determine cache directory".to_string()),
    };
    let workspace_root = workspace_root.join("workspaces");
    let mut cleaned_count = 0;

    if workspace_root.exists() {
        // --- DELETE ORPHAN OR TERMINATED WORKSPACES ---
        if let Ok(entries) = fs::read_dir(&workspace_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let task_id = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned();

                    // If it's a UUID and not in the protected list, it's trash.
                    if !protected_ids.contains(&task_id) {
                        state
                            .logger
                            .log_app("INFO", &format!("Purging workspace: {}", task_id));
                        let _ = fs::remove_dir_all(&path);
                        cleaned_count += 1;
                    }
                }
            }
        }
    }

    // A task is a "ghost" if its workspace is missing and it's not "Queued" or "Starting"
    let mut ghost_ids: Vec<String> = Vec::new();
    for task in all_tasks {
        let path = std::path::Path::new(&task.temp_path);
        // A task is a "ghost" if its workspace is missing and it's not in a state that doesn't need one yet.
        if !path.exists()
            && !matches!(
                task.status,
                crate::state::DownloadStatus::Queued
                    | crate::state::DownloadStatus::Starting
                    | crate::state::DownloadStatus::FetchingMetadata
            )
        {
            ghost_ids.push(task.id.clone());
        }
    }

    for id in &ghost_ids {
        state.remove_task(id);
    }

    if !ghost_ids.is_empty() {
        state.persist_downloads(&app);
    }

    let summary = format!(
        "Cleanup Complete: Removed {} orphan workspaces and {} ghost entries",
        cleaned_count,
        ghost_ids.len()
    );
    state.logger.log_app("INFO", &summary);
    Ok(summary)
}
