//! Core commands for installing and updating dependencies

use crate::dependencies::archive::{extract_deno_from_zip, extract_ffmpeg_from_zip};
use crate::dependencies::fetch::{
    download_file, get_deno_download_url, get_ffmpeg_download_url, get_latest_deno_version,
    get_latest_ytdlp_version, get_ytdlp_download_url,
};
use crate::dependencies::paths::{
    get_bin_dir, get_deno_path, get_ffmpeg_path, get_ffprobe_path, get_ytdlp_path,
};
use crate::state::{AppState, DependencyInstallState};
use std::collections::HashMap;
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager};
use tokio::fs;

#[cfg(target_os = "linux")]
use crate::dependencies::archive::extract_ffmpeg_from_tar_xz;

enum DepStatus {
    Starting,
    Downloading,
    Finalizing,
    Complete,
}

impl DepStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Downloading => "downloading",
            Self::Finalizing => "finalizing",
            Self::Complete => "complete",
        }
    }
}

/// RAII guard to ensure the installing flag is always cleared
struct InstallGuard {
    app: AppHandle,
    target: String,
}

impl InstallGuard {
    fn new(app: AppHandle, target: String) -> Result<Self, String> {
        let normalized_target = match target.as_str() {
            "ffprobe" => "ffmpeg".to_string(),
            other => other.to_string(),
        };

        {
            let state = app.state::<AppState>();
            let mut inner = state.inner.lock().map_err(|_| "Lock error".to_string())?;

            // 1. If "all" is installing, block everything
            if inner.installing.contains("all") {
                return Err("Full dependency installation already in progress".to_string());
            }

            // 2. If this is "all", block if ANY binary is installing
            if normalized_target == "all" && !inner.installing.is_empty() {
                return Err(
                    "Cannot start full installation while individual components are installing"
                        .to_string(),
                );
            }

            // 3. Per-target check
            if inner.installing.contains(&normalized_target) {
                return Err(format!(
                    "Installation for {} is already in progress",
                    normalized_target
                ));
            }

            inner.installing.insert(normalized_target.clone());
        }
        Ok(Self {
            app,
            target: normalized_target,
        })
    }
}

impl Drop for InstallGuard {
    fn drop(&mut self) {
        if let Ok(mut inner) = self.app.state::<AppState>().inner.lock() {
            inner.installing.remove(&self.target);
        }
    }
}

/// Install dependencies (yt-dlp, ffmpeg, and deno).
/// If `target` is `None`, installs all components.
/// This function uses an `InstallGuard` to prevent concurrent overlapping installations.
#[tauri::command]
pub async fn install_dependencies(app: AppHandle, target: Option<String>) -> Result<(), String> {
    let target_str = target.clone().unwrap_or_else(|| "all".to_string());

    // Acquire lock or return early if already installing
    let _guard = match InstallGuard::new(app.clone(), target_str) {
        Ok(g) => g,
        Err(_) => {
            // If already installing, just return early.
            // Note: We don't error out to avoid noisy frontend alerts for double-clicks
            return Ok(());
        }
    };

    // Emit deps status so UI knows installation is in progress
    let _ = check_deps_status(app.clone()).await;

    let state = app.state::<AppState>();
    let event_bus = &state.event_bus;

    // If this is a full install from setup screen, mark setup as shown IMMEDIATELY
    // so if the user quits mid-download, they see the banner next time (not setup screen)
    if target.is_none() {
        let mut config = state
            .inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .config
            .clone();
        config.setup_shown = true;
        config.custom_deps = false;
        crate::config::save_config(app.clone(), state.clone(), config)
            .map_err(|e| e.to_string())?;
    }

    // Resolve bin directory AFTER potential config reset
    let bin_dir = get_bin_dir(&app);

    event_bus.emit(crate::events::AppEvent::Log {
        source: "app".to_string(),
        level: "INFO".to_string(),
        message: "Initializing dependency installation...".to_string(),
    });

    // Reset/Init overall status (optional, but good for UI)
    update_dependency_state(&state, "all", DepStatus::Starting.as_str(), 0.0);

    fs::create_dir_all(&bin_dir).await.map_err(|e| {
        let err_msg = format!("Failed to create bin directory: {}", e);
        event_bus.emit(crate::events::AppEvent::Log {
            source: "app".to_string(),
            level: "ERROR".to_string(),
            message: err_msg.clone(),
        });
        err_msg
    })?;

    let should_install_ytdlp = target.as_deref().is_none_or(|t| t == "yt-dlp");
    let should_install_ffmpeg = target
        .as_deref()
        .is_none_or(|t| t == "ffmpeg" || t == "ffprobe");
    let should_install_deno = target.as_deref().is_none_or(|t| t == "deno");

    if should_install_ytdlp {
        event_bus.emit(crate::events::AppEvent::Log {
            source: "app".to_string(),
            level: "INFO".to_string(),
            message: "Downloading yt-dlp...".to_string(),
        });
        update_dependency_state(&state, "yt-dlp", DepStatus::Downloading.as_str(), 0.0);

        download_ytdlp(&app).await?;
        update_dependency_state(&state, "yt-dlp", DepStatus::Complete.as_str(), 100.0);
    }

    if should_install_ffmpeg {
        event_bus.emit(crate::events::AppEvent::Log {
            source: "app".to_string(),
            level: "INFO".to_string(),
            message: "Downloading ffmpeg...".to_string(),
        });
        update_dependency_state(&state, "ffmpeg", DepStatus::Downloading.as_str(), 0.0);

        download_ffmpeg(&app).await?;
        update_dependency_state(&state, "ffmpeg", DepStatus::Complete.as_str(), 100.0);
    }

    if should_install_deno {
        event_bus.emit(crate::events::AppEvent::Log {
            source: "app".to_string(),
            level: "INFO".to_string(),
            message: "Downloading deno...".to_string(),
        });
        update_dependency_state(&state, "deno", DepStatus::Downloading.as_str(), 0.0);

        download_deno(&app).await?;
        update_dependency_state(&state, "deno", DepStatus::Complete.as_str(), 100.0);
    }

    // Emit completion via bus
    update_dependency_state(&state, "all", DepStatus::Complete.as_str(), 100.0);

    // setup_shown is already set above (before downloads start)
    // No need to set it again here

    // Guard drops here → installing set shrinks.
    // Emit deps status so UI knows if all installs are done
    drop(_guard);
    let _ = check_deps_status(app.clone()).await;

    Ok(())
}

/// Helper to set executable permissions on Unix-like systems
#[cfg(unix)]
async fn set_executable_permission(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(path)
        .await
        .map_err(|e| format!("Failed to get metadata: {}", e))?;

    let mut perms = metadata.permissions();
    perms.set_mode(0o755);

    fs::set_permissions(path, perms)
        .await
        .map_err(|e| format!("Failed to set permissions: {}", e))?;

    Ok(())
}

#[cfg(not(unix))]
async fn set_executable_permission(_path: &Path) -> Result<(), String> {
    Ok(())
}

/// Download yt-dlp
async fn download_ytdlp(app: &AppHandle) -> Result<(), String> {
    let url = get_ytdlp_download_url();
    let dest = get_ytdlp_path(app);
    let event_bus = &app.state::<AppState>().event_bus;

    download_file(url, &dest, "yt-dlp", event_bus).await?;
    update_dependency_state(
        &app.state::<AppState>(),
        "yt-dlp",
        DepStatus::Finalizing.as_str(),
        100.0,
    );
    set_executable_permission(&dest).await?;

    Ok(())
}

/// Download ffmpeg
async fn download_ffmpeg(app: &AppHandle) -> Result<(), String> {
    let url = get_ffmpeg_download_url();
    let bin_dir = get_bin_dir(app);
    let event_bus = &app.state::<AppState>().event_bus;

    // For Windows and macOS, we need to download and extract zip
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        let zip_path = bin_dir.join("ffmpeg.zip");
        // Always try to download first. robust download_file handles overwrite/partial logic now.
        if let Err(e) = download_file(url, &zip_path, "ffmpeg", event_bus).await {
            // If download fails, ensure no bad file is left
            let _ = fs::remove_file(&zip_path).await;
            return Err(e);
        }

        // archive.rs emits real 0–100% extraction progress per-file — no status override needed here
        match extract_ffmpeg_from_zip(app, &zip_path, &bin_dir).await {
            Ok(_) => {
                // Happy path: Clean up zip
                let _ = fs::remove_file(&zip_path).await;
            }
            Err(e) => {
                // If extraction failed (e.g. invalid archive), delete the zip so retry catches it
                let _ = fs::remove_file(&zip_path).await;
                return Err(format!("Failed to extract ffmpeg (archive deleted): {}", e));
            }
        }

        // Make executable on macOS
        #[cfg(target_os = "macos")]
        {
            use crate::dependencies::paths::get_ffmpeg_path;
            use crate::dependencies::paths::get_ffprobe_path;
            set_executable_permission(&get_ffmpeg_path(app)).await?;
            set_executable_permission(&get_ffprobe_path(app)).await?;
        }
    }

    #[cfg(target_os = "linux")]
    {
        let tar_path = bin_dir.join("ffmpeg.tar.xz");
        download_file(url, &tar_path, "ffmpeg", event_bus).await?;

        // archive.rs emits real 0–100% extraction progress per-file — no status override needed here
        extract_ffmpeg_from_tar_xz(app, &tar_path, &bin_dir).await?;

        // Clean up tar.xz
        let _ = fs::remove_file(&tar_path).await;

        use crate::dependencies::paths::get_ffmpeg_path;
        use crate::dependencies::paths::get_ffprobe_path;
        set_executable_permission(&get_ffmpeg_path(app)).await?;
        set_executable_permission(&get_ffprobe_path(app)).await?;
    }

    Ok(())
}

/// Update yt-dlp to latest version
#[tauri::command]
pub async fn update_ytdlp(app: AppHandle) -> Result<(), String> {
    let _guard = match InstallGuard::new(app.clone(), "yt-dlp".to_string()) {
        Ok(g) => g,
        Err(_) => return Ok(()),
    };

    // Emit so UI knows install is in progress
    let _ = check_deps_status(app.clone()).await;

    download_ytdlp(&app).await?;
    update_dependency_state(
        &app.state::<AppState>(),
        "yt-dlp",
        DepStatus::Complete.as_str(),
        100.0,
    );

    drop(_guard);
    let _ = check_deps_status(app.clone()).await;

    Ok(())
}

/// Update Deno to latest version
#[tauri::command]
pub async fn update_deno(app: AppHandle) -> Result<(), String> {
    let _guard = match InstallGuard::new(app.clone(), "deno".to_string()) {
        Ok(g) => g,
        Err(_) => return Ok(()),
    };

    // Emit so UI knows install is in progress
    let _ = check_deps_status(app.clone()).await;

    download_deno(&app).await?;
    update_dependency_state(
        &app.state::<AppState>(),
        "deno",
        DepStatus::Complete.as_str(),
        100.0,
    );

    drop(_guard);
    let _ = check_deps_status(app.clone()).await;

    Ok(())
}

/// Download Deno
async fn download_deno(app: &AppHandle) -> Result<(), String> {
    let url = get_deno_download_url();
    let bin_dir = get_bin_dir(app);
    let event_bus = &app.state::<AppState>().event_bus;

    let zip_path = bin_dir.join("deno.zip");
    if let Err(e) = download_file(url, &zip_path, "deno", event_bus).await {
        let _ = fs::remove_file(&zip_path).await;
        return Err(e);
    }

    // archive.rs emits real 0–100% extraction progress per-file — no status override needed here
    match extract_deno_from_zip(app, &zip_path, &bin_dir).await {
        Ok(_) => {
            let _ = fs::remove_file(&zip_path).await;
        }
        Err(e) => {
            let _ = fs::remove_file(&zip_path).await;
            return Err(format!("Failed to extract deno (archive deleted): {}", e));
        }
    }

    set_executable_permission(&get_deno_path(app)).await?;

    Ok(())
}

/// Dependency status information
#[derive(Clone, serde::Serialize)]
pub struct DependencyStatus {
    pub yt_dlp_installed: bool,
    pub yt_dlp_version: Option<String>,
    pub yt_dlp_update_available: bool,
    pub yt_dlp_latest_version: Option<String>,
    pub ffmpeg_installed: bool,
    pub ffprobe_installed: bool,
    pub deno_installed: bool,
    pub deno_version: Option<String>,
    pub deno_update_available: bool,
    pub deno_latest_version: Option<String>,
    pub binaries_missing: bool,
    pub installation_in_progress: bool,
    pub custom_deps: bool,
    pub setup_shown: bool,
}

/// Run a command silently (no window appears on Windows) and return its trimmed stdout.
/// Used for version checks and diagnostic commands.
async fn run_silent_command(path: &std::path::Path, args: &[&str]) -> Result<String, String> {
    #[cfg(windows)]
    let command = {
        #[allow(unused_imports)]
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        tokio::process::Command::new(path)
            .args(args)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
    };

    #[cfg(not(windows))]
    let command = tokio::process::Command::new(path).args(args).output();

    let output = command.await.map_err(|e| format!("Spawn error: {}", e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(format!("Command failed with status: {}", output.status))
    }
}

/// Check if dependencies are installed and detect their versions.
/// In "Custom Mode", it verifies by running `--version` commands instead of checking paths.
/// Returns a `DependencyStatus` struct containing installation and update information.
#[tauri::command]
pub async fn check_dependencies(app: tauri::AppHandle) -> Result<DependencyStatus, String> {
    let state = app.state::<AppState>();
    let (custom_deps, setup_shown, installation_in_progress) = {
        let inner = state.inner.lock().unwrap_or_else(|e| e.into_inner());
        (
            inner.config.custom_deps,
            inner.config.setup_shown,
            !inner.installing.is_empty(),
        )
    };

    let ytdlp_path = get_ytdlp_path(&app);
    let ffmpeg_path = get_ffmpeg_path(&app);
    let ffprobe_path = get_ffprobe_path(&app);
    let deno_path = get_deno_path(&app);

    let mut yt_dlp_version: Option<String> = None;
    let yt_dlp_installed = {
        let res = run_silent_command(&ytdlp_path, &["--version"]).await;
        if let Ok(ver) = res {
            yt_dlp_version = Some(ver);
            true
        } else {
            false
        }
    };

    let ffmpeg_installed = if custom_deps {
        run_silent_command(&ffmpeg_path, &["-version"])
            .await
            .is_ok()
    } else {
        ffmpeg_path.exists()
    };

    let ffprobe_installed = if custom_deps {
        run_silent_command(&ffprobe_path, &["-version"])
            .await
            .is_ok()
    } else {
        ffprobe_path.exists()
    };

    let deno_installed = if custom_deps {
        run_silent_command(&deno_path, &["--version"]).await.is_ok()
    } else {
        deno_path.exists()
    };

    let mut yt_dlp_update_available = false;
    let mut yt_dlp_latest_version: Option<String> = None;
    let mut deno_version: Option<String> = None;
    let mut deno_update_available = false;
    let mut deno_latest_version: Option<String> = None;

    // Get current Deno version if installed (needed for update check)
    if deno_installed {
        if let Ok(full) = run_silent_command(&deno_path, &["--version"]).await {
            if let Some(first_line) = full.lines().next() {
                let ver = first_line.replace("deno ", "");
                let ver = ver
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                deno_version = Some(ver);
            }
        }
    }

    // 5. Only do full version/update checks if NOT in custom mode
    if !custom_deps {
        let (mut cached_yt, mut cached_deno, is_fresh) = {
            let inner = state.inner.lock().unwrap_or_else(|e| e.into_inner());
            let fresh = inner
                .last_update_check
                .map(|t| t.elapsed().as_secs() < 3600)
                .unwrap_or(false);
            (
                inner.cached_ytdlp_latest.clone(),
                inner.cached_deno_latest.clone(),
                fresh,
            )
        };

        if !is_fresh {
            // Drop lock and fetch fresh versions
            if yt_dlp_installed {
                if let Ok(latest) = get_latest_ytdlp_version().await {
                    cached_yt = Some(latest);
                }
            }
            if deno_installed {
                if let Ok(latest) = get_latest_deno_version().await {
                    cached_deno = Some(latest);
                }
            }

            // Update cache in state
            if let Ok(mut inner) = state.inner.lock() {
                inner.last_update_check = Some(std::time::Instant::now());
                inner.cached_ytdlp_latest = cached_yt.clone();
                inner.cached_deno_latest = cached_deno.clone();
            }
        }

        yt_dlp_latest_version = cached_yt;
        deno_latest_version = cached_deno;

        if let Some(ref latest) = yt_dlp_latest_version {
            if let Some(ref current) = yt_dlp_version {
                yt_dlp_update_available = current != latest;
            }
        }

        if let Some(ref latest) = deno_latest_version {
            if let Some(ref current) = deno_version {
                let latest_clean = latest.trim_start_matches('v');
                deno_update_available = current != latest_clean;
            }
        }
    }

    let binaries_missing =
        !yt_dlp_installed || !ffmpeg_installed || !ffprobe_installed || !deno_installed;

    Ok(DependencyStatus {
        yt_dlp_installed,
        yt_dlp_version,
        yt_dlp_update_available,
        yt_dlp_latest_version,
        ffmpeg_installed,
        ffprobe_installed,
        deno_installed,
        deno_version,
        deno_update_available,
        deno_latest_version,
        binaries_missing,
        installation_in_progress,
        custom_deps,
        setup_shown,
    })
}

/// Command to re-check dependencies and emit status to frontend
#[tauri::command]
pub async fn check_deps_status(app: tauri::AppHandle) -> Result<DependencyStatus, String> {
    let status = check_dependencies(app.clone()).await?;
    let _ = app.emit("app:deps-status-changed", &status);
    Ok(status)
}

#[tauri::command]
pub fn get_dependency_installation_state(
    app: AppHandle,
) -> Result<HashMap<String, DependencyInstallState>, String> {
    Ok(app
        .state::<AppState>()
        .inner
        .lock()
        .map_err(|_| "Lock error".to_string())?
        .dependency_states
        .clone())
}

/// Helper to update state and emit event, ensuring lock is released before emit
fn update_dependency_state(state: &AppState, target: &str, status: &str, progress: f64) {
    // 1. Update HashMap FIRST
    {
        if let Ok(mut inner) = state.inner.lock() {
            inner.dependency_states.insert(
                target.to_string(),
                DependencyInstallState {
                    target: target.to_string(),
                    status: status.to_string(),
                    progress,
                },
            );
        }
    } // ✅ Release lock BEFORE emitting event

    // 2. Emit Event (no lock held)
    state
        .event_bus
        .emit(crate::events::AppEvent::DependencyStatus {
            target: target.to_string(),
            status: status.to_string(),
            progress,
            total_size: None,
            downloaded: None,
        });
}
