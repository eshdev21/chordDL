use crate::state::{AppState, DownloadStatus};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::broadcast;

/// Events that can occur during a download or dependency setup
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum AppEvent {
    /// A new download task has been created
    DownloadCreated {
        id: String,
        title: String,
        media_type: String,
        is_playlist: bool,
        output_path: String,
    },
    /// Download status update (progress, speed, etc.)
    DownloadStateChanged { id: String, status: DownloadStatus },
    /// Title of the download has been resolved
    TitleChanged { id: String, title: String },
    /// Runtime info like PID
    DownloadRuntimeInfo {
        id: String,
        pid: Option<u32>,
        download_path: Option<PathBuf>,
    },
    /// Log message from yt-dlp or internal logic
    Log {
        source: String, // "app", "ytdlp", "stderr"
        level: String,  // "INFO", "ERROR", etc.
        message: String,
    },
    /// Dependency installation events
    #[allow(dead_code)]
    DependencyStatus {
        target: String, // "yt-dlp", "ffmpeg", "deno"
        status: String, // "downloading", "extracting", "complete"
        progress: f64,
        total_size: Option<u64>,
        downloaded: Option<u64>,
    },
}

/// Centralized event distribution system using a broadcast channel.
#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<Arc<AppEvent>>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    pub fn with_default_capacity() -> Self {
        Self::new(1024)
    }

    pub fn emit(&self, event: AppEvent) {
        // We ignore if there are no subscribers
        let _ = self.tx.send(Arc::new(event));
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<AppEvent>> {
        self.tx.subscribe()
    }
}

/// Starts a global listener that processes all AppEvents and updates the application state
/// Spawns a background task to process the internal event bus and update AppState.
pub fn start_global_listener(app: AppHandle) {
    let state = app.state::<AppState>();
    let mut rx = state.event_bus.subscribe();

    tauri::async_runtime::spawn(async move {
        while let Ok(event) = rx.recv().await {
            let app = app.clone();
            let state = app.state::<AppState>();

            match &*event {
                AppEvent::DownloadCreated {
                    id,
                    title,
                    media_type,
                    is_playlist,
                    output_path,
                } => {
                    let _ = app.emit(
                        "download:created",
                        crate::state::TaskCreatedEvent {
                            id: id.clone(),
                            title: title.clone(),
                            media_type: media_type.clone(),
                            is_playlist: *is_playlist,
                            output_path: output_path.clone(),
                        },
                    );
                }
                AppEvent::DownloadStateChanged { id, status } => {
                    state.update_task_status(&app, id, status.clone());
                }
                AppEvent::TitleChanged { id, title } => {
                    state.update_task_title(&app, id, title);
                }
                AppEvent::DownloadRuntimeInfo {
                    id,
                    pid,
                    download_path,
                } => {
                    state.set_task_runtime_info(id, *pid, download_path.clone());
                }
                AppEvent::Log {
                    source,
                    level,
                    message,
                } => {
                    if source == "ytdlp" || source == "stderr" {
                        // Print to terminal for dev visibility
                        println!("[{}] {}", source, message);

                        state.logger.log_ytdlp(message);
                        let _ = app.emit("download:log", format!("[yt-dlp] {}", message));
                    } else {
                        // Include icons for app logs
                        let icon = match level.as_str() {
                            "ERROR" => "❌ ",
                            "WARN" => "⚠ ",
                            "INFO" => "→ ",
                            _ => "",
                        };
                        // Print to terminal for dev visibility
                        println!("[{}{}] {}", icon, level, message);
                        state.logger.log_app(level, message);
                    }
                }
                AppEvent::DependencyStatus {
                    target,
                    status,
                    progress,
                    total_size,
                    downloaded,
                } => {
                    // Emit to frontend
                    let _ = app.emit(
                        "dependency:status",
                        serde_json::json!({
                            "target": target,
                            "status": status,
                            "progress": progress,
                            "total_size": total_size,
                            "downloaded": downloaded,
                        }),
                    );
                }
            }
        }
    });
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(1024)
    }
}
