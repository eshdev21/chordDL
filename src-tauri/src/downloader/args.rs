use crate::dependencies::{get_deno_path, get_ffmpeg_path};
use std::path::PathBuf;

#[allow(clippy::too_many_arguments)]
pub fn build_ytdlp_args(
    app: &tauri::AppHandle,
    url: String,
    media_type: String,
    format: String,
    quality: String,
    is_playlist: bool,
    workspace_path: PathBuf,
    cookies_enabled: bool,
    concurrent_fragments: usize,
) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();

    // Format selection
    if media_type == "video" {
        let format_spec = match format.as_str() {
            "mp4" => format!(
                "bestvideo[ext=mp4][height<={quality}]+bestaudio[ext=m4a]/best[ext=mp4]/best",
                quality = quality
            ),
            "webm" => format!(
                "bestvideo[ext=webm][height<={quality}]+bestaudio[ext=webm]/best",
                quality = quality
            ),
            _ => format!(
                "bestvideo[height<={quality}]+bestaudio/best",
                quality = quality
            ),
        };

        args.extend([
            "-f".to_string(),
            format_spec,
            "--merge-output-format".to_string(),
            format.clone(),
            "--remux-video".to_string(),
            format,
        ]);
    } else {
        match format.as_str() {
            "mp3" | "opus" => {
                args.extend([
                    "-f".to_string(),
                    "bestaudio".to_string(),
                    "--extract-audio".to_string(),
                    "--audio-format".to_string(),
                    format.clone(),
                    "--audio-quality".to_string(),
                    "0".to_string(),
                ]);
            }
            _ => {
                args.extend([
                    "-f".to_string(),
                    "bestaudio/best".to_string(),
                    "--extract-audio".to_string(),
                    "--audio-format".to_string(),
                    format.clone(),
                ]);
            }
        }
    }

    // FFmpeg
    let ffmpeg_path = get_ffmpeg_path(app);
    if let Some(parent) = ffmpeg_path.parent() {
        if ffmpeg_path.exists() {
            args.extend([
                "--ffmpeg-location".to_string(),
                parent.to_string_lossy().to_string(),
            ]);
        }
    }

    // Deno
    let deno_path = get_deno_path(app);
    if deno_path.exists() {
        args.extend([
            "--js-runtimes".to_string(),
            format!("deno:{}", deno_path.to_string_lossy()),
        ]);
    }

    // Common arguments
    args.extend([
        "--embed-metadata".to_string(),
        "--embed-thumbnail".to_string(),
        "--download-archive".to_string(),
        workspace_path
            .join("archive.txt")
            .to_string_lossy()
            .to_string(),
        "--output".to_string(),
        if is_playlist {
            "%(playlist_title|%(channel|%(uploader|Unknown)s)s)s/%(title)s.%(ext)s".to_string()
        } else {
            "%(title)s.%(ext)s".to_string()
        },
        "--progress".to_string(),
        "--newline".to_string(),
        "-P".to_string(),
        format!("home:{}", workspace_path.to_string_lossy()),
        "-P".to_string(),
        format!("temp:{}", workspace_path.to_string_lossy()),
        "--retries".to_string(),
        "infinite".to_string(),
        "--fragment-retries".to_string(),
        "infinite".to_string(),
        "--retry-sleep".to_string(),
        "5".to_string(),
        "--retry-sleep".to_string(),
        "fragment:5".to_string(),
        "--socket-timeout".to_string(),
        "30".to_string(),
        "--concurrent-fragments".to_string(),
        concurrent_fragments.to_string(),
        "--impersonate".to_string(),
        "chrome".to_string(),
        "--extractor-retries".to_string(),
        "5".to_string(),
        "--encoding".to_string(),
        "utf-8".to_string(),
        "--windows-filenames".to_string(),
        "--replace-in-metadata".to_string(),
        "title".to_string(),
        r"[\U00010000-\U0010ffff]".to_string(),
        "".to_string(),
    ]);

    // Use Firefox cookies directly (ONLY if cookies are enabled)
    if cookies_enabled {
        args.extend(["--cookies-from-browser".to_string(), "firefox".to_string()]);
    }

    if is_playlist {
        args.push("--no-abort-on-error".to_string());
    }

    args.push(
        if is_playlist {
            "--yes-playlist"
        } else {
            "--no-playlist"
        }
        .to_string(),
    );
    args.push(url);

    args
}
