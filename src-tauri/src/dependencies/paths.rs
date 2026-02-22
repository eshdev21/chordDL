//! Binary path resolution for dependencies
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// Get the binary directory path
pub fn get_bin_dir(app: &AppHandle) -> PathBuf {
    let state = app.state::<crate::state::AppState>();
    let custom_deps = {
        let inner = state.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.config.custom_deps
    };

    let folder = if custom_deps { "custombin" } else { "bin" };

    app.path()
        .app_local_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(folder)
}

/// Get yt-dlp executable path
pub fn get_ytdlp_path(app: &AppHandle) -> PathBuf {
    let mut path = get_bin_dir(app).join("yt-dlp");
    if cfg!(windows) {
        path.set_extension("exe");
    }
    path
}

/// Get ffmpeg executable path
pub fn get_ffmpeg_path(app: &AppHandle) -> PathBuf {
    let mut path = get_bin_dir(app).join("ffmpeg");
    if cfg!(windows) {
        path.set_extension("exe");
    }
    path
}

/// Get deno executable path
pub fn get_deno_path(app: &AppHandle) -> PathBuf {
    let mut path = get_bin_dir(app).join("deno");
    if cfg!(windows) {
        path.set_extension("exe");
    }
    path
}

/// Get ffprobe executable path
pub fn get_ffprobe_path(app: &AppHandle) -> PathBuf {
    let mut path = get_bin_dir(app).join("ffprobe");
    if cfg!(windows) {
        path.set_extension("exe");
    }
    path
}
