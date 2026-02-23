//! Config Manager - Handles user settings persistence
//!
//! Config is loaded once from disk into AppState on startup,
//! then served from memory. Saves go to both memory and disk.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{Manager, State};

use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// Core application configuration structure for persistence.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AppConfig {
    #[serde(default)]
    pub download_path: Option<String>,
    #[serde(default)]
    pub video_path: Option<String>,
    #[serde(default = "default_format")]
    pub default_format: String,
    #[serde(default = "default_video_format")]
    pub video_format: String,
    #[serde(default = "default_video_quality")]
    pub video_quality: String,
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_downloads: usize,
    #[serde(default)]
    pub cookies_enabled: bool,
    #[serde(default = "default_false")]
    pub debug_logging: bool,
    #[serde(default)]
    pub custom_deps: bool,
    #[serde(default)]
    pub setup_shown: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            download_path: None,
            video_path: None,
            default_format: default_format(),
            video_format: default_video_format(),
            video_quality: default_video_quality(),
            max_concurrent_downloads: default_max_concurrent(),
            cookies_enabled: false,
            debug_logging: false,
            custom_deps: false,
            setup_shown: false,
        }
    }
}

fn default_false() -> bool {
    false
}

fn default_format() -> String {
    "m4a".to_string()
}

fn default_video_format() -> String {
    "mp4".to_string()
}

fn default_video_quality() -> String {
    "1080".to_string()
}

fn default_max_concurrent() -> usize {
    3
}

/// Get config file path using Tauri's standard resolver
fn get_config_path(app: &tauri::AppHandle) -> AppResult<PathBuf> {
    let app_local_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(app_local_dir.join("config.json"))
}

/// Load config from disk (non-command, used internally by state.rs on startup)
pub fn load_config_from_disk(app: &tauri::AppHandle) -> (AppConfig, bool) {
    let path_result = get_config_path(app);
    let mut config = AppConfig::default();
    let mut is_fresh = true;

    if let Ok(path) = path_result {
        if path.exists() {
            is_fresh = false;
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(loaded) = serde_json::from_str(&content) {
                    config = loaded;
                }
            }
        }

        // Sanitize and persist if needed
        if sanitize_config(&mut config) && !is_fresh {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(content) = serde_json::to_string_pretty(&config) {
                let _ = fs::write(&path, content);
            }
        }
    }

    (config, is_fresh)
}

/// Ensure config values are valid, repairing them if necessary.
/// Returns true if any changes were made.
fn sanitize_config(config: &mut AppConfig) -> bool {
    let mut changed = false;

    // Fix 0 concurrency (which blocks downloads forever)
    if config.max_concurrent_downloads == 0 {
        config.max_concurrent_downloads = default_max_concurrent();
        changed = true;
    }

    // Fix missing paths
    if config.download_path.is_none()
        || config
            .download_path
            .as_ref()
            .map(|s| s.is_empty())
            .unwrap_or(false)
    {
        config.download_path = Some(get_default_download_path());
        changed = true;
    }
    if config.video_path.is_none()
        || config
            .video_path
            .as_ref()
            .map(|s| s.is_empty())
            .unwrap_or(false)
    {
        config.video_path = Some(get_default_video_path());
        changed = true;
    }

    // Fix empty formats
    if config.default_format.is_empty() {
        config.default_format = default_format();
        changed = true;
    }
    if config.video_format.is_empty() {
        config.video_format = default_video_format();
        changed = true;
    }
    if config.video_quality.is_empty() {
        config.video_quality = default_video_quality();
        changed = true;
    }

    changed
}

/// Load config — returns cached version from AppState (no disk I/O)
#[tauri::command]
pub fn load_config(state: State<'_, AppState>) -> AppConfig {
    state
        .inner
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .config
        .clone()
}

/// Save config — updates in-memory cache AND persists to disk
#[tauri::command]
pub fn save_config(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    config: AppConfig,
) -> AppResult<()> {
    // Update in-memory cache and semaphore limit
    let limit = config.max_concurrent_downloads;
    let debug_logging = config.debug_logging;

    let (old_limit, old_debug) = {
        let mut inner = state.inner.lock().unwrap_or_else(|e| e.into_inner());
        let old_limit = inner.config.max_concurrent_downloads;
        let old_debug = inner.config.debug_logging;
        inner.config = config.clone();
        (old_limit, old_debug)
    };
    state.update_download_limit(limit);

    if limit != old_limit {
        state.logger.log_app(
            "INFO",
            &format!(
                "Concurrent download limit changed from {} to {}",
                old_limit, limit
            ),
        );
    }

    if debug_logging != old_debug {
        state.logger.log_app(
            "INFO",
            &format!(
                "Debug logging {}",
                if debug_logging { "enabled" } else { "disabled" }
            ),
        );
    }

    // Update logger state
    state.logger.set_debug(&app, debug_logging);

    // Persist to disk
    let path = get_config_path(&app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(AppError::Io)?;
    }

    let content =
        serde_json::to_string_pretty(&config).map_err(|e| AppError::Internal(e.to_string()))?;
    fs::write(&path, content).map_err(AppError::Io)?;

    // Notify frontend
    state.emit_config_change(&app);

    Ok(())
}

/// Get default download path
#[tauri::command]
pub fn get_default_download_path() -> String {
    dirs::audio_dir()
        .or_else(dirs::download_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .to_string_lossy()
        .to_string()
}

/// Get default video download path
#[tauri::command]
pub fn get_default_video_path() -> String {
    dirs::video_dir()
        .or_else(dirs::download_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .to_string_lossy()
        .to_string()
}

/// Open the logs directory in the system file explorer
#[tauri::command]
pub async fn open_app_logs_dir(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;
    let log_dir = app
        .path()
        .app_local_data_dir()
        .map(|dir| dir.join("logs"))
        .map_err(|e| e.to_string())?;
    if !log_dir.exists() {
        std::fs::create_dir_all(&log_dir).map_err(|e| e.to_string())?;
    }

    #[cfg(windows)]
    {
        std::process::Command::new("explorer")
            .arg(&log_dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&log_dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&log_dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}
