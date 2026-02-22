use crate::dependencies::{get_deno_path, get_ytdlp_path};
use crate::error::AppResult;
use crate::state::AppState;
use tauri::{AppHandle, State};
use tokio::process::Command;

const YTDLP_IMPERSONATE_TARGET: &str = "chrome";

#[tauri::command]
pub async fn check_firefox_auth(app: AppHandle, state: State<'_, AppState>) -> AppResult<String> {
    // Guard against multiple simultaneous checks
    {
        let mut inner = state.inner.lock().unwrap_or_else(|e| e.into_inner());
        if !inner.config.cookies_enabled {
            return Ok("Cookies disabled in settings".to_string());
        }
        if inner.cookie_check_in_progress {
            return Ok("Check already in progress".to_string());
        }
        inner.cookie_check_in_progress = true;
    }

    // Capture result using a local variable to ensure we can reset the flag before returning
    let result = perform_auth_check(&app, &state).await;

    // Reset flag
    {
        let mut inner = state.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.cookie_check_in_progress = false;
    }

    match &result {
        Ok(s) => state
            .logger
            .log_app("INFO", &format!("Auth check completed: {:?}", s)),
        Err(e) => state
            .logger
            .log_app("ERROR", &format!("Auth check failed: {}", e)),
    }

    result
}

async fn perform_auth_check(app: &AppHandle, state: &AppState) -> AppResult<String> {
    let ytdlp_path = get_ytdlp_path(app);
    let deno_path = get_deno_path(app);

    if !ytdlp_path.exists() {
        state
            .logger
            .log_app("WARN", "yt-dlp not found for auth check");
        return Ok("yt-dlp missing".to_string());
    }

    // If cookie file doesn't exist, we'll try to use Firefox directly via the fallback in the args.
    // This allows the initial "Auth" check to succeed if Firefox is signed in.

    let mut args = vec![
        "--simulate".to_string(),
        "--cookies-from-browser".to_string(),
        "firefox".to_string(),
        "--impersonate".to_string(),
        YTDLP_IMPERSONATE_TARGET.to_string(),
    ];

    if deno_path.exists() {
        args.extend([
            "--js-runtimes".to_string(),
            format!("deno:{}", deno_path.to_string_lossy()),
        ]);
    }

    // Use a known age-restricted video to ensure the cookies actually grant access to restricted content.
    // "Content Warning" or Age-gated video is required for a true auth check.
    args.push("https://www.youtube.com/watch?v=qpgTC9MDx1o".to_string());

    use std::process::Stdio;
    use tauri::Emitter;
    use tokio::io::{AsyncBufReadExt, BufReader};

    let command_str = format!("{} {}", ytdlp_path.display(), args.join(" "));
    state
        .logger
        .log_app("INFO", &format!("Performing auth check: {}", command_str));
    state
        .logger
        .log_ytdlp(&format!("[auth] >>> RUNNING: {}", command_str));

    let mut cmd = Command::new(ytdlp_path);
    cmd.args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        #[allow(unused_imports)]
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn().map_err(crate::error::AppError::Io)?;

    // Capture stdout/stderr
    let stdout = child
        .stdout
        .take()
        .ok_or(crate::error::AppError::Internal("No stdout".to_string()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or(crate::error::AppError::Internal("No stderr".to_string()))?;

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    // Spawn task to read stdout
    let app_handle = app.clone();
    // Wait, Logger isn't cloneable easily? It's Arc'd in AppState?
    // Actually state is passed as &AppState.
    // I need to use a move block but Logger doesn't implement Clone?
    // Let's check Logger definition.

    let stdout_handle = tokio::spawn(async move {
        while let Ok(Some(line)) = stdout_reader.next_line().await {
            let _ = app_handle.emit("download:log", format!("[auth-check] {}", line));
            // We can't easily log to ytdlp from here without Logger being cloneable or Arc'd.
            // But AppState owns Logger.
        }
    });

    let mut accumulated_stderr = String::new();

    // Read stderr in main flow to accumulate for check
    while let Ok(Some(line)) = stderr_reader.next_line().await {
        let _ = app.emit("download:log", format!("[auth-check] {}", line));
        state.logger.log_ytdlp(&format!("[auth-stderr] {}", line));
        accumulated_stderr.push_str(&line);
        accumulated_stderr.push('\n');
    }

    let status_code = child.wait().await.map_err(crate::error::AppError::Io)?;
    let _ = stdout_handle
        .await
        .map_err(|e| crate::error::AppError::Internal(e.to_string()))?;

    state
        .logger
        .log_ytdlp(&format!(">>> AUTH EXIT: {}", status_code));

    // Detailed Error Analysis
    if accumulated_stderr.contains("Database is locked")
        || accumulated_stderr.contains("sqlite3.OperationalError")
    {
        return Err(crate::error::AppError::YtDlpBrowserLock);
    }

    if status_code.success() {
        Ok("Cookies found ✓".to_string())
    } else if accumulated_stderr.contains("Sign in to confirm") {
        Ok("Sign in required in Firefox".to_string())
    } else {
        Ok("Cookies not found or invalid".to_string())
    }
}
