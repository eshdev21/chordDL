mod config;
mod database;
mod dependencies;
mod downloader;
mod error;
mod events;
mod logger;
mod state;

use config::{
    get_default_download_path, get_default_video_path, load_config, open_app_logs_dir, save_config,
};
use dependencies::{
    check_dependencies, check_deps_status, get_dependency_installation_state, install_dependencies,
    update_deno, update_ytdlp,
};
use downloader::{
    cancel_download, check_firefox_auth, cleanup_orphans, download_single, get_active_downloads,
    get_all_downloads, initialize_app, pause_download, resume_download,
};
use state::AppState;
use std::sync::{Arc, Mutex};
use tauri::Manager;

#[tauri::command]
fn open_folder(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
async fn toggle_custom_mode(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let state = app.state::<AppState>();

    // 1. Update config
    let mut config = state
        .inner
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .config
        .clone();
    config.custom_deps = enabled;
    crate::config::save_config(app.clone(), state.clone(), config).map_err(|e| e.to_string())?;

    // 2. Recheck deps and emit status
    let _ = crate::dependencies::check_deps_status(app.clone()).await;

    Ok(())
}

#[tauri::command]
async fn complete_setup(app: tauri::AppHandle, custom: bool) -> Result<(), String> {
    let state = app.state::<AppState>();

    // Ensure directory exists if custom mode is requested
    if custom {
        let bin_dir = crate::dependencies::get_bin_dir(&app);
        if !bin_dir.exists() {
            std::fs::create_dir_all(&bin_dir).map_err(|e| e.to_string())?;
        }
    }

    let mut config = state
        .inner
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .config
        .clone();
    config.setup_shown = true;
    config.custom_deps = custom;
    crate::config::save_config(app.clone(), state.clone(), config).map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .setup(|app| {
            let handle = app.handle();
            let state = handle.state::<AppState>();

            // 1. Load config first to detect fresh setup
            let (config, is_fresh) = crate::config::load_config_from_disk(handle);

            // 2. Initialize database (fresh or existing)
            let db_result = if is_fresh {
                crate::database::Db::init_fresh(handle, state.logger.clone())
            } else {
                crate::database::Db::new(handle, state.logger.clone())
            };

            match db_result {
                Ok(db) => {
                    state.inner.lock().unwrap_or_else(|e| e.into_inner()).db =
                        Some(Arc::new(Mutex::new(db)));
                }
                Err(e) => {
                    state
                        .logger
                        .log_app("ERROR", &format!("Failed to initialize database: {}", e));
                }
            }

            // 3. Store config in state
            let limit = config.max_concurrent_downloads;
            let debug_logging = config.debug_logging;
            state.inner.lock().unwrap_or_else(|e| e.into_inner()).config = config.clone();
            state.update_download_limit(limit);

            // Initialize logger
            state.logger.init(handle, debug_logging);

            // Log resolved tool paths for debugging
            state.logger.log_app(
                "INFO",
                &format!(
                    "yt-dlp path: {}",
                    crate::dependencies::get_ytdlp_path(handle).display()
                ),
            );
            state.logger.log_app(
                "INFO",
                &format!(
                    "ffmpeg path: {}",
                    crate::dependencies::get_ffmpeg_path(handle).display()
                ),
            );
            state.logger.log_app(
                "INFO",
                &format!(
                    "ffprobe path: {}",
                    crate::dependencies::get_ffprobe_path(handle).display()
                ),
            );
            state.logger.log_app(
                "INFO",
                &format!(
                    "deno path: {}",
                    crate::dependencies::get_deno_path(handle).display()
                ),
            );

            // 4. Load tasks from DB (now that DB is ready)
            state.load_from_disk(handle);

            // 5. Start Global Event Listener
            crate::events::start_global_listener(handle.clone());

            // 6. Check if setup is needed
            let handle_clone = handle.clone();
            tauri::async_runtime::spawn(async move {
                // ALWAYS check deps status on startup so the UI has fresh data (Setup or Dashboard)
                let _ = check_deps_status(handle_clone).await;
            });

            // Run cleanup in background
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let _ = cleanup_orphans(app_handle).await;
            });

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            // Dependencies
            check_dependencies,
            check_deps_status,
            install_dependencies,
            update_ytdlp,
            update_deno,
            get_dependency_installation_state,
            toggle_custom_mode,
            complete_setup,
            // Downloads
            download_single,
            cancel_download,
            pause_download,
            resume_download,
            get_active_downloads,
            get_all_downloads,
            initialize_app,
            check_firefox_auth,
            // Config
            load_config,
            save_config,
            get_default_download_path,
            get_default_video_path,
            // Logs
            open_app_logs_dir,
            open_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
