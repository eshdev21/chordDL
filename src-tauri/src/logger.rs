use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

pub struct LogState {
    pub enabled: bool,
    pub file: Option<File>,
}

/// Core logging engine for managing application and engine-specific logs.
#[derive(Clone)]
pub struct Logger {
    app_log: Arc<Mutex<LogState>>,
    ytdlp_log: Arc<Mutex<LogState>>,
}

fn rotate_log_if_needed(path: &std::path::Path, max_size: u64) {
    if !path.exists() {
        return;
    }
    match std::fs::metadata(path) {
        Ok(meta) if meta.len() > max_size => {
            let filename = path.file_name().unwrap_or_default().to_string_lossy();
            let path_1 = path.with_file_name(format!("{}.1", filename));
            let path_2 = path.with_file_name(format!("{}.2", filename));

            let _ = std::fs::remove_file(&path_2);
            let _ = std::fs::rename(&path_1, &path_2);
            let _ = std::fs::rename(path, &path_1);
        }
        _ => {}
    }
}

impl Logger {
    pub fn new() -> Self {
        Self {
            app_log: Arc::new(Mutex::new(LogState {
                enabled: false,
                file: None,
            })),
            ytdlp_log: Arc::new(Mutex::new(LogState {
                enabled: false,
                file: None,
            })),
        }
    }

    /// Initialize log files and rotation settings based on debug mode.
    pub fn init(&self, app: &AppHandle, debug: bool) {
        if !debug {
            let mut app_s = self.app_log.lock().unwrap_or_else(|e| e.into_inner());
            app_s.enabled = false;
            let mut ytdlp_s = self.ytdlp_log.lock().unwrap_or_else(|e| e.into_inner());
            ytdlp_s.enabled = false;
            return;
        }

        let log_dir = match app.path().app_local_data_dir() {
            Ok(dir) => dir.join("logs"),
            Err(_) => return,
        };

        let _ = std::fs::create_dir_all(&log_dir);

        rotate_log_if_needed(&log_dir.join("app.log"), 2_000_000); // 2MB
        rotate_log_if_needed(&log_dir.join("ytdlp.log"), 5_000_000); // 5MB

        {
            let mut app_s = self.app_log.lock().unwrap_or_else(|e| e.into_inner());
            app_s.enabled = true;
            app_s.file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_dir.join("app.log"))
                .ok();
        }

        {
            let mut ytdlp_s = self.ytdlp_log.lock().unwrap_or_else(|e| e.into_inner());
            ytdlp_s.enabled = true;
            ytdlp_s.file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_dir.join("ytdlp.log"))
                .ok();
        }

        self.log_app("INFO", "Logging initialized.");
    }

    pub fn set_debug(&self, app: &AppHandle, enabled: bool) {
        let (was_enabled, is_enabled) = {
            let mut app_s = self.app_log.lock().unwrap_or_else(|e| e.into_inner());
            let old = app_s.enabled;
            app_s.enabled = enabled;
            (old, enabled)
        };

        if is_enabled && !was_enabled {
            self.init(app, true);
        } else if !is_enabled {
            let mut app_s = self.app_log.lock().unwrap_or_else(|e| e.into_inner());
            app_s.file = None;
            let mut ytdlp_s = self.ytdlp_log.lock().unwrap_or_else(|e| e.into_inner());
            ytdlp_s.enabled = false;
            ytdlp_s.file = None;
        }
    }

    /// Write a structured message to the main application log.
    pub fn log_app(&self, level: &str, msg: &str) {
        let mut app_s = self.app_log.lock().unwrap_or_else(|e| e.into_inner());
        if !app_s.enabled {
            return;
        }

        if let Some(file) = app_s.file.as_mut() {
            let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            let line = format!("[{}] {:<5} | {}\n", timestamp, level, msg);
            let _ = file.write_all(line.as_bytes());
            let _ = file.flush();
        }
    }

    /// Write a raw message to the dedicated yt-dlp technical log.
    pub fn log_ytdlp(&self, msg: &str) {
        let mut ytdlp_s = self.ytdlp_log.lock().unwrap_or_else(|e| e.into_inner());
        if !ytdlp_s.enabled {
            return;
        }

        if let Some(file) = ytdlp_s.file.as_mut() {
            let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            let line = format!("[{}] {}\n", timestamp, msg);
            let _ = file.write_all(line.as_bytes());
            let _ = file.flush();
        }
    }
}
