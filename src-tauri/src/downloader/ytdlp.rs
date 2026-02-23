use crate::error::AppError;
use crate::state::DownloadStatus;
use regex::Regex;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Run yt-dlp and parse its output
pub async fn run_ytdlp(
    task_id: &str,
    ytdlp_path: &PathBuf,
    args: Vec<String>,
    output_dir: &str,
    event_bus: &crate::events::EventBus,
) -> Result<String, crate::error::AppError> {
    let command_str = format!("{} {}", ytdlp_path.display(), args.join(" "));
    event_bus.emit(crate::events::AppEvent::Log {
        source: "app".to_string(),
        level: "INFO".to_string(),
        message: format!("Running yt-dlp: {}", command_str),
    });
    event_bus.emit(crate::events::AppEvent::Log {
        source: "ytdlp".to_string(),
        level: "INFO".to_string(),
        message: format!(">>> RUNNING: {}", command_str),
    });

    let mut cmd = Command::new(ytdlp_path);
    cmd.args(&args)
        .current_dir(output_dir)
        .env("PYTHONIOENCODING", "utf-8")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        #[allow(unused_imports)]
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            AppError::DependencyMissing("yt-dlp".to_string())
        } else {
            AppError::Io(e)
        }
    })?;

    // Store PID in state
    if let Some(pid) = child.id() {
        event_bus.emit(crate::events::AppEvent::DownloadRuntimeInfo {
            id: task_id.to_string(),
            pid: Some(pid),
            download_path: None,
        });
    }

    // Signal that we're now fetching metadata (before any output is parsed)
    event_bus.emit(crate::events::AppEvent::DownloadStateChanged {
        id: task_id.to_string(),
        status: DownloadStatus::FetchingMetadata,
    });

    let stdout = child
        .stdout
        .take()
        .ok_or(AppError::Internal("Failed to capture stdout".to_string()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or(AppError::Internal("Failed to capture stderr".to_string()))?;

    // Spawn a separate task to read stderr concurrently
    let event_bus_stderr = event_bus.clone();
    let task_id_stderr = task_id.to_string();
    let stderr_handle = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        let mut full_stderr = String::new();

        while let Ok(Some(line)) = reader.next_line().await {
            // Log line immediately
            event_bus_stderr.emit(crate::events::AppEvent::Log {
                source: "stderr".to_string(),
                level: "ERROR".to_string(),
                message: line.clone(),
            });

            // Check for retry messages
            // Example: "[download] Got error: ... Retrying (1/inf)..."
            if line.contains("Retrying")
                || (line.contains("Got error") && line.contains("download"))
            {
                event_bus_stderr.emit(crate::events::AppEvent::DownloadStateChanged {
                    id: task_id_stderr.clone(),
                    status: DownloadStatus::Downloading {
                        progress: 0.0,
                        speed: "Network Error".to_string(),
                        eta: "Retrying...".to_string(),
                        playlist: None,
                    },
                });
            }

            full_stderr.push_str(&line);
            full_stderr.push('\n');
        }
        full_stderr
    });

    let mut reader = BufReader::new(stdout);
    let mut line_buffer = Vec::new();

    let mut current_title = "Downloading...".to_string();
    let mut last_filename = String::new();

    let mut current_item = 1;
    let mut total_items = 1;

    // Throttling state
    let mut last_update_time = std::time::Instant::now();
    let throttle_interval = std::time::Duration::from_millis(100);

    // 3. Read and parse stdout line by line (Resilient to encoding errors)
    while let Ok(n) = reader.read_until(b'\n', &mut line_buffer).await {
        if n == 0 {
            break;
        }

        let line = String::from_utf8_lossy(&line_buffer).trim().to_string();
        line_buffer.clear();

        event_bus.emit(crate::events::AppEvent::Log {
            source: "ytdlp".to_string(),
            level: "STDOUT".to_string(),
            message: line.clone(),
        });

        let (title_update, status_update) = parse_ytdlp_output_line(&line, &current_title);

        if let Some(new_title) = title_update {
            current_title = new_title.clone();
            last_filename = new_title.clone(); // ← ADD THIS LINE
            event_bus.emit(crate::events::AppEvent::TitleChanged {
                id: task_id.to_string(),
                title: new_title,
            });
        }

        if let Some(mut status) = status_update {
            // Merge metadata logic...
            match &mut status {
                DownloadStatus::Downloading { playlist, .. }
                | DownloadStatus::Merging { playlist, .. }
                | DownloadStatus::Finalizing { playlist, .. } => {
                    if let Some(p) = playlist {
                        if p.current_index > 0 {
                            current_item = p.current_index;
                        } else {
                            p.current_index = current_item;
                        }
                        if p.total_items > 0 {
                            total_items = p.total_items;
                        } else {
                            p.total_items = total_items;
                        }
                        if p.item_title == "Starting item..." || p.item_title.is_empty() {
                            p.item_title = current_title.clone();
                        }
                    } else if total_items > 1 {
                        *playlist = Some(crate::state::PlaylistMetadata {
                            current_index: current_item,
                            total_items,
                            item_title: current_title.clone(),
                        });
                    }
                }
                _ => {}
            }

            if let DownloadStatus::Downloading {
                ref mut speed,
                ref mut eta,
                ..
            } = status
            {
                if speed == "Unknown B/s" {
                    *speed = "---".to_string();
                }
                if eta == "Unknown" {
                    *eta = "---".to_string();
                }
            }

            // Capture final filename from Merger line (video downloads)
            // This overwrites the audio-stream destination, giving us the true merged output file
            if let DownloadStatus::Merging {
                playlist: Some(ref meta),
            } = status
            {
                if !meta.item_title.is_empty() && meta.item_title != "Downloading..." {
                    last_filename = meta.item_title.clone();
                }
            }

            let should_update = match &status {
                DownloadStatus::Downloading { progress, .. } => {
                    *progress >= 100.0 || last_update_time.elapsed() >= throttle_interval
                }
                _ => true,
            };

            if should_update {
                event_bus.emit(crate::events::AppEvent::DownloadStateChanged {
                    id: task_id.to_string(),
                    status,
                });
                last_update_time = std::time::Instant::now();
            }
        }
    }

    // Wait for process to complete
    let proc_status = child.wait().await.map_err(AppError::Io)?;

    // Await the stderr task to get the full error string
    let full_stderr = stderr_handle
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Clear runtime info
    event_bus.emit(crate::events::AppEvent::DownloadRuntimeInfo {
        id: task_id.to_string(),
        pid: None,
        download_path: None,
    });

    if proc_status.success() {
        if last_filename.is_empty() {
            last_filename = current_title;
        }
        Ok(last_filename)
    } else {
        // Analyze stderr for specific errors
        if full_stderr.contains("ffprobe not found") {
            Err(AppError::DependencyMissing("ffprobe".to_string()))
        } else if full_stderr.contains("ffmpeg not found") {
            Err(AppError::DependencyMissing("ffmpeg".to_string()))
        } else if full_stderr.contains("No supported JavaScript runtime could be found") {
            Err(AppError::DependencyMissing("deno".to_string()))
        } else if full_stderr.contains("Sign in to confirm you are not a bot")
            || full_stderr.contains("Sign in to confirm your age")
        {
            Err(AppError::YtDlpLoginRequired)
        } else if full_stderr.contains("Database is locked")
            || full_stderr.contains("sqlite3.OperationalError")
        {
            Err(AppError::YtDlpBrowserLock)
        } else if full_stderr.contains("HTTP Error 429") {
            Err(AppError::YtDlpRateLimited)
        } else if full_stderr.contains("Video unavailable")
            || full_stderr.contains("This video is unavailable")
        {
            Err(AppError::YtDlpUnavailable)
        } else if (full_stderr.contains("fragment") && full_stderr.contains("not found"))
            || full_stderr.contains("Could not resolve host")
        {
            Err(AppError::YtDlpNetworkError)
        } else {
            let error_msg = if full_stderr.trim().is_empty() {
                "Unknown yt-dlp error".to_string()
            } else {
                full_stderr.trim().to_string()
            };
            Err(AppError::YtDlp(error_msg))
        }
    }
}

/// Parse a single line of yt-dlp output and return potential title and status updates
fn parse_ytdlp_output_line(
    line: &str,
    current_title: &str,
) -> (Option<String>, Option<DownloadStatus>) {
    use std::sync::OnceLock;

    // Optimization: Regexes are only initialized if the line prefix matches.
    // We group regexes by their expected prefix to avoid scanning every line with every regex.

    static PROGRESS_RE: OnceLock<Regex> = OnceLock::new();
    static SIMPLE_PROGRESS_RE: OnceLock<Regex> = OnceLock::new();
    static PLAYLIST_RE: OnceLock<Regex> = OnceLock::new();
    static ITEM_COUNT_RE: OnceLock<Regex> = OnceLock::new();
    static ALREADY_DOWNLOADED_RE: OnceLock<Regex> = OnceLock::new();
    static DESTINATION_RE: OnceLock<Regex> = OnceLock::new();
    static MERGE_RE: OnceLock<Regex> = OnceLock::new();
    static POSTPROCESS_RE: OnceLock<Regex> = OnceLock::new();

    let mut title_update = None;
    let mut status_update = None;

    // 1. [download] lines (Most common, check first)
    if let Some(rest) = line.strip_prefix("[download] ") {
        // [download] Downloading item 10 of 39
        if let Some(caps) = ITEM_COUNT_RE
            .get_or_init(|| Regex::new(r"^Downloading item (\d+) of (\d+)$").unwrap())
            .captures(rest)
        {
            let current = caps[1].parse::<usize>().unwrap_or(1);
            let total = caps[2].parse::<usize>().unwrap_or(1);
            return (
                None,
                Some(DownloadStatus::Downloading {
                    progress: 0.0,
                    speed: "—".to_string(),
                    eta: "Starting...".to_string(),
                    playlist: Some(crate::state::PlaylistMetadata {
                        current_index: current,
                        total_items: total,
                        item_title: "Starting item...".to_string(),
                    }),
                }),
            );
        }

        // [download]   2.3% of 10.00MiB at 1.2MiB/s ETA 00:05
        if let Some(caps) = PROGRESS_RE
            .get_or_init(|| Regex::new(r"^\s+(\d+\.?\d*)%.*at\s+(.+?)\s+ETA\s+(.+)").unwrap())
            .captures(rest)
        {
            status_update = Some(DownloadStatus::Downloading {
                progress: caps[1].parse::<f64>().unwrap_or(0.0),
                speed: caps[2].trim().to_string(),
                eta: caps[3].trim().to_string(),
                playlist: None, // Will be merged with current_item in run_ytdlp if needed
            });
            return (title_update, status_update);
        }

        // [download] 100% of 10.00MiB
        if rest.trim_start().starts_with("100% of") {
            status_update = Some(DownloadStatus::Downloading {
                progress: 100.0,
                speed: "—".to_string(),
                eta: "Done".to_string(),
                playlist: None,
            });
            return (title_update, status_update);
        }

        // [download]   2.3%
        if let Some(caps) = SIMPLE_PROGRESS_RE
            .get_or_init(|| Regex::new(r"^\s+(\d+\.?\d*)%").unwrap())
            .captures(rest)
        {
            let progress = caps[1].parse::<f64>().unwrap_or(0.0);
            status_update = Some(DownloadStatus::Downloading {
                progress,
                speed: if progress >= 100.0 { "—" } else { "..." }.to_string(),
                eta: if progress >= 100.0 { "Done" } else { "..." }.to_string(),
                playlist: None,
            });
            return (title_update, status_update);
        }

        // [download] Destination: ...
        if let Some(caps) = DESTINATION_RE
            .get_or_init(|| Regex::new(r"^Destination:\s+(.+)$").unwrap())
            .captures(rest)
        {
            let path_str = caps[1].trim();
            // Get just the filename (last part after \ or /)
            let name = path_str
                .rsplit_once(['\\', '/'])
                .map(|(_, name)| name)
                .unwrap_or(path_str);

            title_update = Some(name.to_string());
            return (title_update, status_update);
        }

        // [download] Downloading playlist: ...
        if let Some(caps) = PLAYLIST_RE
            .get_or_init(|| Regex::new(r"^Downloading playlist:\s+(.+)$").unwrap())
            .captures(rest)
        {
            title_update = Some(caps[1].trim().to_string());
            return (title_update, status_update);
        }

        // [download] ... has already been downloaded
        if let Some(caps) = ALREADY_DOWNLOADED_RE
            .get_or_init(|| Regex::new(r"^\s+(.+)\s+has already been downloaded").unwrap())
            .captures(rest)
        {
            let item_name = std::path::Path::new(&caps[1])
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| caps[1].to_string());

            status_update = Some(DownloadStatus::Downloading {
                progress: 100.0,
                speed: "—".to_string(),
                eta: "Done".to_string(),
                playlist: Some(crate::state::PlaylistMetadata {
                    current_index: 0, // Unknown here, but usually followed by ITEM_COUNT
                    total_items: 0,
                    item_title: item_name,
                }),
            });
            return (title_update, status_update);
        }
    }

    // 2. [Merger] lines
    if let Some(rest) = line.strip_prefix("[Merger] ") {
        if let Some(caps) = MERGE_RE
            .get_or_init(|| Regex::new(r#"^Merging formats into "(.+)"$"#).unwrap())
            .captures(rest)
        {
            let item_title = std::path::Path::new(&caps[1])
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| caps[1].to_string());

            status_update = Some(DownloadStatus::Merging {
                playlist: Some(crate::state::PlaylistMetadata {
                    current_index: 0,
                    total_items: 0,
                    item_title,
                }),
            });
            return (title_update, status_update);
        }
    }

    // 3. Post-processing lines (Metadata, Fixup, etc.)
    // These often don't have a uniform prefix like "[PostProcess]", but specific tool prefixes.
    // [FixupM4a], [MetadataParser], etc.
    // Checking for '[' at start is a good heuristic.
    if line.starts_with('[')
        && POSTPROCESS_RE
            .get_or_init(|| {
                Regex::new(r"^\[(?:ExtractAudio|FixupM4a|Fixup|ModifyChapters|EmbedThumbnail|MetadataParser)\]")
                    .unwrap()
            })
            .is_match(line)
    {
        status_update = Some(DownloadStatus::Merging {
            playlist: Some(crate::state::PlaylistMetadata {
                current_index: 0,
                total_items: 0,
                item_title: current_title.to_string(),
            }),
        });
        return (title_update, status_update);
    }

    // Fallback: Check for "Destination:" without [download] prefix (rare but possible in some versions/configs?)
    // Or other miscellaneous lines.
    // Preserving original generic DESTINATION_RE check just in case, but strictly anchored.
    // Actually, `Destination:` usually follows `[download]`. The original code had `r"Destination: (.+)$"` which matched anywhere?
    // Let's stick to the prefix-based for now. If needed we can add a catch-all.

    (title_update, status_update)
}
