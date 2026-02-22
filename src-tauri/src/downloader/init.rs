use crate::error::AppResult;
use crate::state::{AppState, DownloadTask};
use tauri::{AppHandle, Manager};

/// Initialize app — called once on startup, returns only verified resumable tasks
#[tauri::command]
pub fn get_active_downloads(app: AppHandle) -> Vec<DownloadTask> {
    let state = app.state::<AppState>();

    // Return all tasks regardless of state so user can manage them (Retry/Cancel)
    state.get_all_tasks()
}

#[tauri::command]
pub async fn initialize_app(app: AppHandle) -> AppResult<Vec<DownloadTask>> {
    // Return the active downloads so frontend can render the queue
    Ok(get_active_downloads(app))
}
